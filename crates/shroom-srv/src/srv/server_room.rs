use std::sync::{atomic::AtomicBool, Arc};

use shroom_pkt::Packet;

use crate::{
    actor::{TickActor, TickActorHandle, TickActorRunner},
    util::interval::GameInterval,
    Context, ServerId,
};

use super::{
    room_set::{RoomSessionSet, ServerSessionData, SessionSet},
    server_session::SessionHandler,
    server_socket::ServerSocketHandle,
};

#[derive(Debug)]
struct Shared {
    pending_shutdown: AtomicBool,
    should_shutdown: AtomicBool,
}

impl Shared {
    fn new() -> Self {
        Self {
            pending_shutdown: AtomicBool::new(false),
            should_shutdown: AtomicBool::new(false),
        }
    }

    pub fn has_pending_shutdown(&self) -> bool {
        self.pending_shutdown
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn cancel_shutdown(&self) {
        self.should_shutdown
            .fetch_and(false, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn start_shutdown(&self) {
        self.should_shutdown
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.pending_shutdown
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn reset_pending_shutdown(&self) {
        self.pending_shutdown
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn can_shutdown(&self) -> bool {
        self.pending_shutdown
            .load(std::sync::atomic::Ordering::SeqCst)
            && self
                .should_shutdown
                .load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[derive(Debug)]
pub struct ServerRoomHandle<H: RoomHandler> {
    handle: TickActorHandle<Room<H>>,
    shared: Arc<Shared>,
}

impl<H: RoomHandler> ServerRoomHandle<H> {
    pub fn join(&self, session: ServerSessionData<H::SessionHandler>) -> anyhow::Result<()> {
        self.shared.cancel_shutdown();
        self.handle
            .tx
            .try_send(ServerRoomMsg::AddSession(session))
            .unwrap();
        Ok(())
    }

    pub fn cancelled_shutdown(&self) -> bool {
        if !self.shared.can_shutdown() {
            self.shared.reset_pending_shutdown();
            return true;
        }

        false
    }

    pub fn reset_pending_shutdown(&self) {
        self.shared.reset_pending_shutdown();
    }

    pub fn shutdown(&self) {
        self.handle.cancel();
    }

    pub fn spawn(room: H, ctx: H::Ctx) -> Self {
        let shared = Arc::new(Shared::new());
        let room = Room::new(room, shared.clone());
        let handle = TickActorRunner::spawn(room, ctx);
        Self { handle, shared }
    }
}

pub enum ServerRoomMsg<H: RoomHandler> {
    AddSession(ServerSessionData<H::SessionHandler>),
}

impl<H: RoomHandler> std::fmt::Debug for ServerRoomMsg<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AddSession(_) => f.debug_tuple("AddSession").finish(),
        }
    }
}

pub struct RoomCtx<'ctx, H: RoomSessionHandler> {
    pub room_ctx: &'ctx mut <H::RoomHandler as RoomHandler>::Ctx,
    pub room: &'ctx mut H::RoomHandler,
    pub room_sessions: RoomSessionSet<H>,
}

impl<'ctx, H: RoomSessionHandler> Context for RoomCtx<'ctx, H> {
    fn create(_clock_ref: crate::util::clock::GameClockRef) -> Self {
        unimplemented!()
    }

    fn time(&self) -> crate::util::clock::GameTime {
        self.room_ctx.time()
    }

    fn wait_tick(&mut self) -> impl futures::prelude::Future<Output = ()> + Send {
        self.room_ctx.wait_tick()
    }
}

pub trait RoomSessionHandler: Sized + Send + 'static {
    type RoomHandler: RoomHandler;
    type Msg: Send + 'static;
    type SessionId: ServerId;
    type RoomId: ServerId;

    fn session_id(&self) -> Self::SessionId;
    fn room_id(&self) -> Self::RoomId;

    fn on_enter_room(
        &mut self,
        sck: &mut ServerSocketHandle,
        ctx: &mut RoomCtx<Self>,
    ) -> anyhow::Result<()>;

    fn on_switch_room(
        ctx: &mut RoomCtx<'_, Self>,
        session: ServerSessionData<Self>,
        new_room: Self::RoomId,
    ) -> anyhow::Result<()>;

    fn on_packet(
        &mut self,
        sck: &mut ServerSocketHandle,
        ctx: &mut RoomCtx<Self>,
        packet: Packet,
    ) -> anyhow::Result<()>;
    fn on_update(
        &mut self,
        sck: &mut ServerSocketHandle,
        ctx: &mut RoomCtx<Self>,
    ) -> anyhow::Result<()>;
    fn on_msg(
        &mut self,
        sck: &mut ServerSocketHandle,
        ctx: &mut RoomCtx<Self>,
        msg: Self::Msg,
    ) -> anyhow::Result<()>;
}

impl<T: RoomSessionHandler> SessionHandler for T {
    type Ctx<'ctx> = RoomCtx<'ctx, T>;
    type Msg = T::Msg;
    type SessionId = T::SessionId;

    fn session_id(&self) -> Self::SessionId {
        self.session_id()
    }

    fn on_packet(
        &mut self,
        sck: &mut ServerSocketHandle,
        ctx: &mut Self::Ctx<'_>,
        packet: Packet,
    ) -> anyhow::Result<()> {
        self.on_packet(sck, ctx, packet)
    }

    fn on_update(
        &mut self,
        sck: &mut ServerSocketHandle,
        ctx: &mut Self::Ctx<'_>,
    ) -> anyhow::Result<()> {
        self.on_update(sck, ctx)
    }

    fn on_msg(
        &mut self,
        sck: &mut ServerSocketHandle,
        ctx: &mut Self::Ctx<'_>,
        msg: Self::Msg,
    ) -> anyhow::Result<()> {
        self.on_msg(sck, ctx, msg)
    }
}

pub trait RoomContext: Context {
    type RoomId: ServerId;

    fn send_shutdown_req(&mut self, room_id: Self::RoomId) -> anyhow::Result<()>;
}

pub trait RoomHandler: Sized + Send + 'static {
    type RoomId: ServerId;
    type Ctx: RoomContext<RoomId = Self::RoomId> + Send;
    type SessionHandler: RoomSessionHandler<RoomHandler = Self, RoomId = Self::RoomId>
        + Send
        + 'static;

    fn room_id(&self) -> Self::RoomId;

    fn on_enter(
        &mut self,
        ctx: &mut Self::Ctx,
        session: &mut ServerSessionData<Self::SessionHandler>,
    ) -> anyhow::Result<()>;
    fn on_leave(
        ctx: &mut RoomCtx<'_, Self::SessionHandler>,
        id: <Self::SessionHandler as RoomSessionHandler>::SessionId,
    ) -> anyhow::Result<()>;
    fn on_update(ctx: &mut RoomCtx<'_, Self::SessionHandler>) -> anyhow::Result<()>;
}

pub struct Room<H: RoomHandler> {
    handler: H,
    update_interval: GameInterval,
    session_set: SessionSet<H::SessionHandler>,
    empty_counter: usize,
    shared: Arc<Shared>,
}

impl<H: RoomHandler> std::fmt::Debug for Room<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Room")
            .field("update_interval", &self.update_interval)
            //.field("session_set", &self.session_set)
            .field("empty_counter", &self.empty_counter)
            .finish()
    }
}

impl<H> TickActor for Room<H>
where
    H: RoomHandler,
{
    type Msg = ServerRoomMsg<H>;
    type Ctx<'ctx> = H::Ctx where <H as RoomHandler>::Ctx: 'ctx;

    fn on_msg(&mut self, ctx: &mut Self::Ctx<'_>, msg: Self::Msg) -> anyhow::Result<()> {
        match msg {
            ServerRoomMsg::AddSession(session) => {
                self.on_add(session, ctx)?;
            }
        }
        Ok(())
    }

    fn on_update(&mut self, ctx: &mut Self::Ctx<'_>) -> anyhow::Result<()> {
        let t = ctx.time();
        if self.update_interval.update(t) {
            self.check_shutdown(ctx)?;

            H::on_update(&mut self.session_set.room_ctx(&mut self.handler, ctx))?;
            self.session_set.update(&mut self.handler, ctx)?;
        }

        Ok(())
    }
}

impl<H> Room<H>
where
    H: RoomHandler,
{
    fn new(handler: H, shared: Arc<Shared>) -> Self {
        Self {
            handler,
            update_interval: GameInterval::new(1),
            session_set: SessionSet::new(),
            empty_counter: 0,
            shared,
        }
    }

    fn check_shutdown(&mut self, ctx: &mut H::Ctx) -> anyhow::Result<()> {
        if self.session_set.is_empty() {
            self.empty_counter += 1;
        } else {
            self.empty_counter = 0;
        }

        // Only start a shutdown when none is pending
        if self.empty_counter > 50 && !self.shared.has_pending_shutdown() {
            log::info!("Shutting down field");
            self.shared.start_shutdown();
            ctx.send_shutdown_req(self.handler.room_id())?;
        }
        Ok(())
    }

    fn on_add(
        &mut self,
        mut session: ServerSessionData<H::SessionHandler>,
        ctx: &mut H::Ctx,
    ) -> anyhow::Result<()> {
        self.handler.on_enter(ctx, &mut session)?;
        self.session_set.add(session, &mut self.handler, ctx)?; //TODO

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use turmoil::Builder;

    use crate::{
        srv::room_set::ServerSessionData,
        util::test_util::{
            accept_mock_session, bind, run_mock_client, test_clock, MockCtx, MockHandler,
            MockRoomHandler,
        },
        Context,
    };

    use super::ServerRoomHandle;

    #[test]
    fn echo() {
        let mut sim = Builder::new().build();

        sim.host("server", || async move {
            let clock = test_clock();
            let ctx = MockCtx::create(clock.clone());
            let room_handle = ServerRoomHandle::spawn(MockRoomHandler::default(), ctx);

            let listener = bind().await?;
            let mut id = 0;
            loop {
                id += 1;
                let session = accept_mock_session(
                    &listener,
                    MockHandler {
                        acc: 0,
                        session_id: id,
                    },
                )
                .await?;
                let session = ServerSessionData::new(session);
                room_handle.join(session).unwrap();
            }
        });

        sim.client("client", async {
            run_mock_client().await?;
            Ok(())
        });

        sim.run().unwrap();
    }
}
