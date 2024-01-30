use shroom_pkt::{pkt::EncodeMessage, Packet};
use tokio::sync::mpsc;

use crate::{
    actor::{TickActor, TickActorRunner},
    util::{encode_buffer::EncodeBuf, interval::GameInterval},
    Context, ServerId, MSG_LIMIT_PER_TICK,
};

use super::server_socket::ServerSocketHandle;

pub trait SessionMsg: Sized {
    fn from_packet(packet: Packet);
}

#[derive(Debug)]
pub struct SessionHandle<H: SessionHandler> {
    pub tx: mpsc::Sender<H::Msg>,
}

impl<H: SessionHandler> Clone for SessionHandle<H> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

pub trait SessionHandler: Sized + Send + 'static {
    type Ctx<'ctx>: Context + Send + 'ctx;
    type Msg: Send + 'static;
    type SessionId: ServerId;

    fn session_id(&self) -> Self::SessionId;

    fn on_packet(
        &mut self,
        sck: &mut ServerSocketHandle,
        ctx: &mut Self::Ctx<'_>,
        packet: Packet,
    ) -> anyhow::Result<()>;
    fn on_update(
        &mut self,
        sck: &mut ServerSocketHandle,
        ctx: &mut Self::Ctx<'_>,
    ) -> anyhow::Result<()>;
    fn on_msg(
        &mut self,
        sck: &mut ServerSocketHandle,
        ctx: &mut Self::Ctx<'_>,
        msg: Self::Msg,
    ) -> anyhow::Result<()>;
}

#[derive(Debug)]
pub struct ServerSession<H: SessionHandler> {
    pub socket: ServerSocketHandle,
    pub handler: H,
    encode_buf: EncodeBuf,
    update_interval: GameInterval,
}

pub type SessionActor<H> = TickActorRunner<ServerSession<H>>;

impl<H: SessionHandler> TickActor for ServerSession<H> {
    type Msg = H::Msg;

    type Ctx<'a> = H::Ctx<'a>;

    fn on_msg(&mut self, ctx: &mut Self::Ctx<'_>, msg: Self::Msg) -> anyhow::Result<()> {
        self.handler.on_msg(&mut self.socket, ctx, msg)
    }

    fn on_update(&mut self, ctx: &mut Self::Ctx<'_>) -> anyhow::Result<()> {
        let time = ctx.time();
        for _ in 0..MSG_LIMIT_PER_TICK {
            match self.socket.try_recv() {
                Ok(packet) => {
                    self.handler.on_packet(&mut self.socket, ctx, packet)?;
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    break;
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    return Err(anyhow::anyhow!("rx closed"));
                }
            }
        }

        if self.update_interval.update(time) {
            self.handler.on_update(&mut self.socket, ctx)?;
        }

        Ok(())
    }
}

impl<H> ServerSession<H>
where
    H: SessionHandler,
{
    pub fn new(handler: H, socket_handle: ServerSocketHandle) -> anyhow::Result<Self> {
        Ok(Self {
            update_interval: GameInterval::new(1),
            socket: socket_handle,
            encode_buf: EncodeBuf::new(),
            handler,
        })
    }

    pub fn send_packet(&mut self, pkt: Packet) -> anyhow::Result<()> {
        self.socket
            .tx_send
            .try_send(pkt.into())
            .map_err(|_| anyhow::format_err!("Unable to send"))?;
        Ok(())
    }

    pub fn send_encode_packet(&mut self, pkt: impl EncodeMessage) -> anyhow::Result<()> {
        let pkt = self.encode_buf.encode_onto(pkt)?;
        self.send_packet(pkt)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use turmoil::Builder;

    use crate::{
        actor::TickActor,
        util::test_util::{
            accept_mock_session, bind, run_mock_client, test_clock, MockCtx, MockSessionHandler,
        },
        Context,
    };

    #[test]
    fn echo() {
        let mut sim = Builder::new().build();

        sim.host("server", || async {
            let clock = test_clock();
            let listener = bind().await?;
            loop {
                let mut ctx = MockCtx::create(clock);
                let mut session = accept_mock_session(
                    &listener,
                    MockSessionHandler {
                        acc: 0,
                        session_id: 0,
                    },
                )
                .await?;

                loop {
                    session.on_update(&mut ctx)?;
                    ctx.wait_tick().await;
                }
            }
        });

        sim.client("client", async {
            run_mock_client().await?;
            Ok(())
        });

        sim.run().unwrap();
    }
}
