use tokio::sync::mpsc;

use crate::{
    net::socket::SocketHandle, util::interval::GameInterval, Context, World, MSG_LIMIT_PER_TICK,
};

use super::Packet;

pub struct SessionHandle<Msg> {
    _tx: mpsc::Sender<Msg>,
}

pub trait SessionHandler: Sized + Send + 'static {
    type Ctx<'ctx>: Context + Send + 'ctx;
    type Msg: Send + 'static;

    fn on_packet(
        &mut self,
        sck: &mut SocketHandle,
        ctx: &mut Self::Ctx<'_>,
        packet: Packet,
    ) -> anyhow::Result<()>;
    fn on_update(&mut self, sck: &mut SocketHandle, ctx: &mut Self::Ctx<'_>) -> anyhow::Result<()>;
    fn on_msg(
        &mut self,
        sck: &mut SocketHandle,
        ctx: &mut Self::Ctx<'_>,
        msg: Self::Msg,
    ) -> anyhow::Result<()>;
}

pub struct Session<H: SessionHandler> {
    pub socket: SocketHandle,
    pub handler: H,
    update_interval: GameInterval,
    rx_msg: mpsc::Receiver<H::Msg>,
    tx_msg: mpsc::Sender<H::Msg>,
}

impl<H> Session<H>
where
    H: SessionHandler,
{
    pub async fn new(handler: H, socket_handle: SocketHandle) -> anyhow::Result<Self> {
        let (tx_msg, rx_session) = mpsc::channel(16);
        Ok(Self {
            rx_msg: rx_session,
            update_interval: GameInterval::new(1),
            socket: socket_handle,
            handler,
            tx_msg,
        })
    }

    pub fn on_update(&mut self, ctx: &mut H::Ctx<'_>) -> anyhow::Result<()> {
        let time = ctx.world().time();
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

        while let Ok(msg) = self.rx_msg.try_recv() {
            self.handler.on_msg(&mut self.socket, ctx, msg)?;
        }

        if self.update_interval.update(time) {
            self.handler.on_update(&mut self.socket, ctx)?;
        }

        Ok(())
    }

    pub fn send_packet(&mut self, pkt: Packet) -> anyhow::Result<()> {
        self.socket.tx_send.try_send(pkt)?;
        Ok(())
    }

    pub fn send_msg(&mut self, msg: H::Msg) -> anyhow::Result<()> {
        self.tx_msg
            .try_send(msg)
            .map_err(|_| anyhow::anyhow!("tx closed"))?; //TODO handle full
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use turmoil::Builder;

    use crate::{
        net::socket::SocketHandle,
        util::test_util::{
            bind, run_mock_client, test_clock, MockCodec, MockCtx, MockSessionHandler,
        },
        Context, World,
    };

    use super::Session;

    #[test]
    fn echo() {
        let mut sim = Builder::new().build();

        sim.host("server", || async {
            let clock = test_clock();
            let listener = bind().await?;
            loop {
                let io = listener.accept().await?;
                let socket = SocketHandle::new_server::<MockCodec>(io.0).await?;
                let mut ctx = MockCtx::create(clock);
                let mut session =
                    Session::<MockSessionHandler>::new(MockSessionHandler { acc: 0 }, socket)
                        .await?;

                loop {
                    session.on_update(&mut ctx)?;
                    ctx.world_mut().wait_tick().await;
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
