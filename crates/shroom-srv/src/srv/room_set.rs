use std::{collections::HashMap, ptr::NonNull};

use shroom_pkt::{pkt::EncodeMessage, util::packet_buf::PacketBuf, Packet};
use tokio::sync::mpsc;

use crate::{actor::TickActorRunner, util::encode_buffer::EncodeBuf};

use super::{
    server_room::{RoomCtx, RoomHandler, RoomSessionHandler},
    server_session::{ServerSession, SessionActor, SessionHandle},
    server_socket::PacketMsgTx,
};

#[derive(Debug)]
pub enum PktMsg {
    Packet(Packet),
    PacketBuf(PacketBuf),
}

impl From<Packet> for PktMsg {
    fn from(pkt: Packet) -> Self {
        Self::Packet(pkt)
    }
}

impl From<PacketBuf> for PktMsg {
    fn from(pkt: PacketBuf) -> Self {
        Self::PacketBuf(pkt)
    }
}

pub struct ServerSessionData<H: RoomSessionHandler> {
    session: SessionActor<H>,
    session_handle: SessionHandle<H>,
    tx: PacketMsgTx,
    error: bool,
}

impl<H: RoomSessionHandler> std::fmt::Debug for ServerSessionData<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerSessionData")
            // TODO .field("session", &self.session)
            // TODO .field("session_handle", &self.session_handle)
            .field("tx", &self.tx)
            .field("error", &self.error)
            .finish()
    }
}

impl<H: RoomSessionHandler> ServerSessionData<H> {
    pub fn new(session: ServerSession<H>) -> Self {
        let tx_pkt = session.socket.tx_send.clone();
        let (tx, rx) = mpsc::channel(16);
        let session = TickActorRunner::new(session, rx);
        Self {
            session,
            session_handle: SessionHandle { tx },
            tx: tx_pkt,
            error: false,
        }
    }

    pub fn switch_to(self, ctx: &mut RoomCtx<'_, H>, new_room: H::RoomId) -> anyhow::Result<()> {
        H::on_switch_room(ctx, self, new_room)?;
        Ok(())
    }

    pub fn session_id(&self) -> H::SessionId {
        self.session.inner().handler.session_id()
    }

    pub fn room_id(&self) -> H::RoomId {
        self.session.inner().handler.room_id()
    }

    pub fn handle(&self) -> SessionHandle<H> {
        self.session_handle.clone()
    }

    pub fn update(&mut self, ctx: &mut RoomCtx<'_, H>) -> bool {
        if let Err(err) = self.session.run_once(ctx) {
            log::info!("Error: {:?}", err);
            self.error = true;
            false
        } else {
            true
        }
    }

    pub fn on_enter(&mut self, ctx: &mut RoomCtx<'_, H>) -> anyhow::Result<()> {
        let sess = self.session.inner_mut();
        sess.handler.on_enter_room(&mut sess.socket, ctx)?;

        Ok(())
    }

    pub fn handler_mut(&mut self) -> &mut H {
        &mut self.session.inner_mut().handler
    }

    fn send_packet(&mut self, pkt: Packet) {
        if self.tx.try_send(pkt.into()).is_err() {
            self.error = true;
        }
    }

    fn send_packet_buf(&mut self, pkt: PacketBuf) {
        if self.tx.try_send(pkt.into()).is_err() {
            self.error = true;
        }
    }
}

pub struct RoomSessionSet<H: RoomSessionHandler>(NonNull<SessionSet<H>>);

unsafe impl<H: RoomSessionHandler + Send> Send for RoomSessionSet<H> {}
unsafe impl<H: RoomSessionHandler + Sync> Sync for RoomSessionSet<H> {}

impl<H: RoomSessionHandler> RoomSessionSet<H> {
    fn with_sessions<U>(&mut self, f: impl FnOnce(&mut SessionSet<H>) -> U) -> U {
        // Safety, `RoomSessions` is only constructed
        // during the update loop, so we know nothing can modify It
        let sessions = unsafe { self.0.as_mut() };
        f(sessions)
    }

    pub fn send_buf_to(&mut self, id: H::SessionId, pkt: PacketBuf) {
        self.with_sessions(|sessions| {
            if let Some(sess) = sessions.sessions.get_mut(&id) {
                sess.send_packet_buf(pkt);
            }
        });
    }

    pub fn send_to(&mut self, id: H::SessionId, pkt: Packet) {
        self.with_sessions(|sessions| {
            if let Some(sess) = sessions.sessions.get_mut(&id) {
                sess.send_packet(pkt);
            }
        });
    }

    pub fn send_to_encode(
        &mut self,
        id: H::SessionId,
        data: impl EncodeMessage,
    ) -> anyhow::Result<()> {
        self.with_sessions(|sessions| {
            let pkt = sessions.encode_inner(data)?;
            if let Some(sess) = sessions.sessions.get_mut(&id) {
                sess.send_packet(pkt);
            }
            Ok(())
        })
    }

    pub fn broadcast_filter(&mut self, pkt: Packet, filter: H::SessionId) {
        self.with_sessions(|sessions| sessions.broadcast_filter(pkt, filter));
    }

    pub fn broadcast(&mut self, pkt: Packet) {
        self.with_sessions(|sessions| sessions.broadcast(pkt));
    }

    pub fn broadcast_encode(&mut self, data: impl EncodeMessage) -> anyhow::Result<()> {
        self.with_sessions(|sessions| {
            let pkt = sessions.encode_inner(data)?;
            sessions.broadcast(pkt);
            Ok(())
        })
    }

    pub fn broadcast_encode_filter(
        &mut self,
        data: impl EncodeMessage,
        filter: H::SessionId,
    ) -> anyhow::Result<()> {
        self.with_sessions(|sessions| {
            let pkt = sessions.encode_inner(data)?;
            sessions.broadcast_filter(pkt, filter);
            Ok(())
        })
    }

    pub fn register_transition(&mut self, id: H::SessionId, room: H::RoomId) {
        self.with_sessions(|sessions| sessions.register_transition(id, room));
    }
}

#[derive(Debug)]
pub struct SessionSet<H: RoomSessionHandler> {
    sessions: HashMap<H::SessionId, ServerSessionData<H>>,
    encode_buf: EncodeBuf,
    err_sessions: Vec<H::SessionId>,
    switch_sessions: HashMap<H::SessionId, H::RoomId>,
}

impl<H: RoomSessionHandler> Default for SessionSet<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: RoomSessionHandler> SessionSet<H> {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            encode_buf: EncodeBuf::new(),
            err_sessions: Vec::new(),
            switch_sessions: HashMap::new(),
        }
    }

    pub fn register_transition(&mut self, id: H::SessionId, room: H::RoomId) {
        self.switch_sessions.insert(id, room);
    }

    fn encode_inner(&mut self, data: impl EncodeMessage) -> anyhow::Result<Packet> {
        self.encode_buf.encode_onto(data)
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn add(
        &mut self,
        mut session: ServerSessionData<H>,
        room: &mut H::RoomHandler,
        ctx: &mut <H::RoomHandler as RoomHandler>::Ctx,
    ) -> anyhow::Result<()> {
        let session_ptr = unsafe { NonNull::new_unchecked(self) };
        session.on_enter(&mut RoomCtx {
            room_ctx: ctx,
            room,
            room_sessions: RoomSessionSet(session_ptr),
        })?;
        let id = session.session_id();
        self.sessions.insert(id, session);
        Ok(())
    }

    pub fn send_buf_to(&mut self, id: H::SessionId, pkt: PacketBuf) {
        if let Some(sess) = self.sessions.get_mut(&id) {
            sess.send_packet_buf(pkt);
        }
    }

    pub fn broadcast(&mut self, pkt: Packet) {
        self.sessions
            .values_mut()
            .for_each(|sess| sess.send_packet(pkt.clone()));
    }

    pub fn broadcast_filter(&mut self, pkt: Packet, filter: H::SessionId) {
        self.sessions
            .values_mut()
            .filter(|s| s.session_id() != filter)
            .for_each(|sess| sess.send_packet(pkt.clone()));
    }

    pub(crate) fn room_ctx<'ctx>(
        &'ctx mut self,
        room: &'ctx mut H::RoomHandler,
        ctx: &'ctx mut <H::RoomHandler as RoomHandler>::Ctx,
    ) -> RoomCtx<'ctx, H> {
        let session_ptr = unsafe { NonNull::new_unchecked(self) };
        RoomCtx {
            room_ctx: ctx,
            room,
            room_sessions: RoomSessionSet(session_ptr),
        }
    }

    pub fn update(
        &mut self,
        room: &mut H::RoomHandler,
        ctx: &mut <H::RoomHandler as RoomHandler>::Ctx,
    ) -> anyhow::Result<()> {
        // Safety a non-ZST is always a valid non null reference
        let session_ptr = unsafe { NonNull::new_unchecked(self) };
        for sess in self.sessions.values_mut() {
            if !sess.update(&mut RoomCtx {
                room_ctx: ctx,
                room,
                room_sessions: RoomSessionSet(session_ptr),
            }) {
                self.err_sessions.push(sess.session_id());
            }
        }
        self.remove_error_sessions();

        // To switches here
        for (id, room_id) in self.switch_sessions.drain() {
            if let Some(sess) = self.sessions.remove(&id) {
                sess.switch_to(
                    &mut RoomCtx {
                        room_ctx: ctx,
                        room,
                        room_sessions: RoomSessionSet(session_ptr),
                    },
                    room_id,
                )?;
            }
        }
        Ok(())
    }

    pub fn remove_error_sessions(&mut self) {
        for sess in self.err_sessions.drain(..) {
            self.sessions.remove(&sess);

            //TODO close sessions here
        }
    }
}
