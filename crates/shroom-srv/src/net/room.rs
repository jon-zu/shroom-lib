use std::{ptr::NonNull, time::Duration};

use bytes::BytesMut;
use tokio::sync::mpsc;

use crate::{
    util::{
        interval::GameInterval,
        supervised_task::{SupervisedTask, SupervisedTaskHandle},
    },
    Context, SessionId, World, MSG_LIMIT_PER_TICK,
};

use super::{
    session::{Session, SessionHandler},
    EncodePacket, Packet,
};

struct RoomTask<H: RoomHandler>(Room<H>);


impl<H> LocalSupervisedTask for RoomTask<H>
where
    H: RoomHandler + Send + 'static,
{
    type Context = H::Ctx;

    async fn run(
        &mut self,
        ctx: &mut H::Ctx,
    ) -> impl futures::Future<Output = Result<(), anyhow::Error>> + Send {
        self.0.exec(ctx)
    }
}

pub struct RoomHandle<H: RoomHandler> {
    pub(crate) tx: mpsc::Sender<RoomMsg<H>>,
    task: SupervisedTaskHandle,
}

impl<H: RoomHandler> RoomHandle<H> {
    pub async fn send(&self, msg: RoomMsg<H>) -> anyhow::Result<()> {
        self.tx.send(msg).await.unwrap();
        Ok(())
    }

    pub fn is_finished(&self) -> bool {
        self.task.is_finished()
    }
}

pub enum RoomMsg<H: RoomHandler> {
    AddSession(Session<H::SessionHandler>),
}

struct SessionData<H: RoomHandler> {
    session: Session<H::SessionHandler>,
    tx: mpsc::Sender<Packet>,
    error: bool,
}

pub struct RoomSessions<H: RoomHandler>(NonNull<SessionSet<H>>);

unsafe impl<H: RoomHandler + Send> Send for RoomSessions<H> {}
unsafe impl<H: RoomHandler + Sync> Sync for RoomSessions<H> {}

impl<H: RoomHandler> RoomSessions<H> {
    pub fn broadcast(&mut self, pkt: Packet) {
        unsafe { self.0.as_mut() }.broadcast(pkt);
    }

    pub fn broadcast_encode(&mut self, data: impl EncodePacket) -> anyhow::Result<()> {
        let sessions = unsafe { self.0.as_mut() };
        data.encode(&mut sessions.buf)?;
        let pkt = Packet(sessions.buf.split().freeze());
        sessions.broadcast(pkt);
        Ok(())
    }
}

pub struct SessionSet<H: RoomHandler> {
    sessions: Vec<SessionData<H>>,
    buf: BytesMut,
}

impl<H: RoomHandler> Default for SessionSet<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: RoomHandler> SessionSet<H> {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            buf: BytesMut::with_capacity(2048),
        }
    }

    pub fn add(&mut self, session: Session<H::SessionHandler>) -> anyhow::Result<()> {
        let tx = session.socket.tx_send.clone();
        self.sessions.push(SessionData {
            session,
            tx,
            error: false,
        });
        Ok(())
    }

    pub fn broadcast(&mut self, pkt: Packet) {
        for sess in self.sessions.iter_mut() {
            if sess.tx.try_send(pkt.clone()).is_err() {
                sess.error = true;
            }
        }
    }

    pub fn update(&mut self, room: &mut H, ctx: &mut H::Ctx) -> anyhow::Result<()> {
        let session_ptr = unsafe { NonNull::new_unchecked(self) };
        for sess in self.sessions.iter_mut() {
            sess.session.on_update(&mut RoomCtx {
                room_ctx: ctx,
                room,
                room_sessions: RoomSessions(session_ptr),
            })?;
        }
        Ok(())
    }
}

pub struct RoomCtx<'ctx, H: RoomHandler> {
    pub room_ctx: &'ctx mut H::Ctx,
    pub room: &'ctx mut H,
    pub room_sessions: RoomSessions<H>,
}

impl<'ctx, H: RoomHandler> Context for RoomCtx<'ctx, H> {
    type World = <H::Ctx as Context>::World;

    fn create(_clock_ref: crate::util::clock::GameClockRef) -> Self {
        unimplemented!()
    }

    fn world_mut(&mut self) -> &mut Self::World {
        self.room_ctx.world_mut()
    }

    fn world(&self) -> &Self::World {
        self.room_ctx.world()
    }
}

pub trait RoomHandler: Sized {
    type Ctx: Context + Send;
    type SessionHandler: for<'ctx> SessionHandler<Ctx<'ctx> = RoomCtx<'ctx, Self>> + Send + 'static;

    fn on_enter(
        &mut self,
        ctx: &mut Self::Ctx,
        session: &mut Session<Self::SessionHandler>,
    ) -> anyhow::Result<()>;
    fn on_leave(&mut self, ctx: &mut Self::Ctx, id: SessionId) -> anyhow::Result<()>;
    fn on_update(&mut self, ctx: &mut Self::Ctx) -> anyhow::Result<()>;
}

pub struct Room<H: RoomHandler> {
    handler: H,
    rx: mpsc::Receiver<RoomMsg<H>>,
    update_interval: GameInterval,
    session_set: SessionSet<H>,
    empty_counter: usize,
}

impl<H> Room<H>
where
    H: RoomHandler + Send + 'static,
{
    pub fn new(handler: H, rx: mpsc::Receiver<RoomMsg<H>>) -> Self {
        Self {
            rx,
            handler,
            update_interval: GameInterval::new(1),
            session_set: SessionSet::new(),
            empty_counter: 0,
        }
    }

    fn on_add(
        &mut self,
        mut session: Session<H::SessionHandler>,
        ctx: &mut H::Ctx,
    ) -> anyhow::Result<()> {
        self.handler.on_enter(ctx, &mut session)?;
        self.session_set.add(session)?;

        Ok(())
    }

    pub fn spawn(msg_cap: usize, handler: H, ctx: H::Ctx) -> RoomHandle<H> {
        let (tx, rx) = mpsc::channel(msg_cap);
        let room = Room::new(handler, rx);
        let task = SupervisedTaskHandle::spawn(RoomTask(room), ctx, Duration::from_millis(100));

        RoomHandle { tx, task }
    }

    pub async fn exec(&mut self, ctx: &mut H::Ctx) -> anyhow::Result<()> {
        loop {
            self.on_update(ctx)?;
            ctx.world_mut().wait_tick().await;
            // TODO: configure the decay tick rate
            if self.empty_counter > 100 {
                break Ok(());
            }
        }
    }

    fn process_msg(&mut self, ctx: &mut H::Ctx) -> anyhow::Result<()> {
        for _ in 0..MSG_LIMIT_PER_TICK {
            match self.rx.try_recv() {
                Ok(RoomMsg::AddSession(session)) => {
                    self.on_add(session, ctx)?;
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    break;
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    return Err(anyhow::anyhow!("rx closed"));
                }
            }
        }
        Ok(())
    }

    fn on_update(&mut self, ctx: &mut H::Ctx) -> anyhow::Result<()> {
        let t = ctx.world().time();
        if self.update_interval.update(t) {
            if self.session_set.sessions.is_empty() {
                self.empty_counter += 1;
            } else {
                self.empty_counter = 0;
            }

            // Handle incoming messages
            self.process_msg(ctx)?;
            // Update room
            self.handler.on_update(ctx)?;
            self.session_set.update(&mut self.handler, ctx)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use turmoil::Builder;

    use crate::{
        net::socket::SocketHandle,
        util::test_util::{
            bind, run_mock_client, test_clock, MockCodec, MockCtx, MockHandler, MockRoomHandler,
        },
        Context,
    };

    use super::{Room, RoomMsg, Session};

    #[test]
    fn echo() {
        let mut sim = Builder::new().build();

        sim.host("server", || async move {
            let clock = test_clock();
            let ctx = MockCtx::create(clock.clone());
            let room = Room::spawn(32, MockRoomHandler::default(), ctx);

            let listener = bind().await?;
            loop {
                let io = listener.accept().await?;
                let socket = SocketHandle::new_server::<MockCodec>(io.0).await?;
                let session = Session::<MockHandler>::new(MockHandler { acc: 0 }, socket).await?;
                room.tx.send(RoomMsg::AddSession(session)).await.unwrap();
            }
        });

        sim.client("client", async {
            run_mock_client().await?;
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok(())
        });

        sim.run().unwrap();
    }
}
