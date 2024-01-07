use std::{
    collections::{btree_map::Entry, BTreeMap},
    ops::Deref,
    sync::Arc,
};

use futures::{Future, Stream, StreamExt};
use shroom_net::codec::ShroomCodec;
use tokio::{
    net::{TcpListener, TcpStream, ToSocketAddrs},
    sync::mpsc,
};
use tokio_stream::wrappers::TcpListenerStream;

use crate::{
    util::clock::{GameClock, GameClockRef},
    Context, ServerId,
};

use super::{
    room_set::ServerSessionData,
    server_room::{RoomHandler, RoomSessionHandler, ServerRoomHandle},
    server_session::{ServerSession, SessionHandle},
    server_socket::ServerSocketHandle,
};

pub enum SystemMsg<H: SystemHandler> {
    AddSession(ServerSession<H::SessionHandler>),
    ChangeRoom(ServerSessionData<H::SessionHandler>, H::RoomId),
    ShutdownRoom(H::RoomId),
}

pub type ServerSystemTx<H> = mpsc::UnboundedSender<SystemMsg<H>>;

pub trait SystemHandler: Sized {
    type Ctx: Context + Send + 'static;
    type Msg: Send + 'static;

    type RoomId: ServerId;

    type SessionHandler: RoomSessionHandler<RoomId = Self::RoomId> + Send + 'static;
    type RoomHandler: RoomHandler<Ctx = Self::Ctx, SessionHandler = Self::SessionHandler>
        + Sized
        + Send
        + 'static;

    fn create_session(
        ctx: &Self::Ctx,
        sck: &mut ServerSocketHandle,
    ) -> impl Future<Output = anyhow::Result<Self::SessionHandler>> + Send;
    fn create_room(&mut self, room_id: Self::RoomId) -> anyhow::Result<Self::RoomHandler>;
    fn create_ctx(
        &mut self,
        clock: GameClockRef,
        tx: ServerSystemTx<Self>,
    ) -> anyhow::Result<Self::Ctx>;

    fn on_update(&mut self, ctx: &mut Self::Ctx) -> anyhow::Result<()>;
}

pub struct ServerSystem<H: SystemHandler> {
    sessions: BTreeMap<
        <H::SessionHandler as RoomSessionHandler>::SessionId,
        SessionHandle<H::SessionHandler>,
    >,
    rooms: BTreeMap<H::RoomId, ServerRoomHandle<H::RoomHandler>>,
    clock: GameClock,
    tx: mpsc::UnboundedSender<SystemMsg<H>>,
    rx: mpsc::UnboundedReceiver<SystemMsg<H>>,
    handler: H,
}

impl<H> ServerSystem<H>
where
    H: SystemHandler,
{
    pub fn new(handler: H) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            sessions: BTreeMap::new(),
            rooms: BTreeMap::new(),
            clock: GameClock::default(),
            tx,
            rx,
            handler,
        }
    }

    fn get_or_create_room(
        &mut self,
        room_id: H::RoomId,
    ) -> anyhow::Result<&mut ServerRoomHandle<H::RoomHandler>> {
        match self.rooms.entry(room_id) {
            Entry::Occupied(entry) => return Ok(entry.into_mut()),
            Entry::Vacant(v) => {
                let room = self.handler.create_room(room_id)?;
                let ctx = self
                    .handler
                    .create_ctx(self.clock.handle(), self.tx.clone())?;
                Ok(v.insert(ServerRoomHandle::spawn(room, ctx)))
            }
        }
    }

    async fn handle_msg(&mut self, msg: SystemMsg<H>) -> anyhow::Result<()> {
        match msg {
            SystemMsg::AddSession(session) => {
                let session = ServerSessionData::new(session);
                let id = session.session_id();
                self.sessions.insert(id, session.handle());
                let room = self.get_or_create_room(session.room_id())?;
                room.join(session)?;
            }
            SystemMsg::ChangeRoom(session, room_id) => {
                let room = self.get_or_create_room(room_id)?;
                room.join(session)?;
            }
            SystemMsg::ShutdownRoom(room_id) => match self.rooms.entry(room_id) {
                // Only shutdown the room when it was not cancelled
                Entry::Occupied(room) if !room.get().cancelled_shutdown() => {
                    room.remove().shutdown();
                }
                _ => {
                    log::warn!("Tried to shutdown room but it doesn't exist");
                }
            },
        }

        Ok(())
    }

    pub fn tx(&self) -> mpsc::UnboundedSender<SystemMsg<H>> {
        self.tx.clone()
    }

    pub fn create_acceptor<C>(&mut self, codec: C) -> anyhow::Result<ServerAcceptor<H, C>>
    where
        C: ShroomCodec + Send + Sync + 'static,
        H: 'static,
    {
        let clock = self.clock.handle();
        let ctx = self.handler.create_ctx(clock, self.tx.clone())?;
        Ok(ServerAcceptor::new(self.tx(), ctx, codec))
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let mut ctx = self
            .handler
            .create_ctx(self.clock.handle(), self.tx.clone())?;

        loop {
            tokio::select! {
                msg = self.rx.recv() => {
                    self.handle_msg(msg.expect("Msg always exists")).await?;
                },
                _ = self.clock.tick() => {
                    self.handler.on_update(&mut ctx)?;
                },
            }
        }
    }
}

pub struct ServerAcceptor<H: SystemHandler, C> {
    ctx: Arc<H::Ctx>,
    codec: Arc<C>,
    tx: mpsc::UnboundedSender<SystemMsg<H>>,
}

impl<H: SystemHandler + 'static, C: ShroomCodec + Send + Sync + 'static> ServerAcceptor<H, C> {
    pub fn new(tx: mpsc::UnboundedSender<SystemMsg<H>>, ctx: H::Ctx, codec: C) -> Self {
        Self {
            ctx: Arc::new(ctx),
            tx,
            codec: Arc::new(codec),
        }
    }

    pub async fn run<S>(&mut self, mut io_stream: S) -> anyhow::Result<()>
    where
        S: Stream<Item = Result<C::Transport, std::io::Error>> + Unpin,
        H::Ctx: Sync,
    {
        while let Some(io) = io_stream.next().await {
            let io = io?;
            let tx = self.tx.clone();
            let cdc = self.codec.clone();
            let ctx = self.ctx.clone();

            tokio::spawn(async move {
                let mut socket_handle = ServerSocketHandle::new_server(cdc.deref(), io)
                    .await
                    .unwrap();
                let session_handler = H::create_session(&ctx, &mut socket_handle).await.unwrap();
                let session = ServerSession::new(session_handler, socket_handle).unwrap();
                let _ = tx.send(SystemMsg::AddSession(session));
            });
        }

        Ok(())
    }

    pub async fn run_tcp(&mut self, addr: impl ToSocketAddrs) -> anyhow::Result<()>
    where
        C: ShroomCodec<Transport = TcpStream>,
        H::Ctx: Sync,
    {
        let listener = TcpListener::bind(addr).await?;
        let listener_stream = TcpListenerStream::new(listener);
        self.run(listener_stream).await
    }

    #[cfg(test)]
    pub async fn run_turmoil_tcp(
        &mut self,
        listener: turmoil::net::TcpListener,
    ) -> anyhow::Result<()>
    where
        C: ShroomCodec<
            Transport = shroom_net::codec::LocalShroomTransport<turmoil::net::TcpStream>,
        >,
        H::Ctx: Sync,
    {
        use futures::stream;
        use shroom_net::codec::LocalShroomTransport;

        let listener_stream = stream::unfold(listener, |listener| async move {
            let res = listener
                .accept()
                .await
                .map(|(s, _)| LocalShroomTransport(s));
            Some((res, listener))
        });
        let listener_stream = std::pin::pin!(listener_stream);
        self.run(listener_stream).await
    }
}

#[cfg(test)]
mod tests {
    use std::iter;

    use futures::future;
    use turmoil::Builder;

    use crate::util::test_util::{bind, run_mock_client, MockCodec, MockSystemHandler};

    use super::*;

    #[test]
    fn echo() {
        let mut sim = Builder::new().build();

        sim.host("server", || async move {
            let system = MockSystemHandler;
            let mut system = ServerSystem::new(system);
            let mut acceptor = system.create_acceptor(MockCodec::default()).unwrap();
            let listener = bind().await?;
            let _ = tokio::spawn(async move {
                acceptor.run_turmoil_tcp(listener).await.unwrap();
            });
            system.run().await.unwrap();

            Ok(())
        });

        sim.client("client", async {
            future::try_join_all(iter::repeat_with(|| run_mock_client()).take(10))
                .await
                .unwrap();
            Ok(())
        });

        sim.run().unwrap();
    }
}
