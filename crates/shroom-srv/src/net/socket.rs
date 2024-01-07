use std::net::SocketAddr;

use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use super::{Codec, EncodePacket, Packet};

#[derive(Debug)]
pub struct Socket<C> {
    codec: C,
}

impl<C: Codec> Socket<C> {
    pub async fn handle(self, tx: mpsc::Sender<Packet>, rx: mpsc::Receiver<Packet>) {
        let (mut r, mut w) = self.codec.split();
        let mut tx_r = tx;
        let mut rx_w = rx;
        tokio::select! {
            v = Self::write(&mut rx_w, &mut w) => {
                log::info!("write: {:?}", v);
            },
            v = Self::read(&mut tx_r, &mut r) => {
                log::info!("read: {:?}", v);
            },
        }

        rx_w.close();
    }

    async fn write(rx: &mut mpsc::Receiver<Packet>, w: &mut C::Tx) -> Result<(), C::Error> {
        while let Some(msg) = rx.recv().await {
            let _ = w.send(msg).await;
        }
        Ok(())
    }

    async fn read(tx: &mut mpsc::Sender<Packet>, r: &mut C::Rx) -> Result<(), C::Error> {
        while let Some(msg) = r.next().await {
            // TODO handle out of space and tx dropped
            if let Err(err) = tx.send(msg?).await {
                log::error!("rx error: {:?}", err);
                break;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct SocketHandle {
    pub(crate) tx_send: mpsc::Sender<Packet>,
    pub(crate) rx_recv: mpsc::Receiver<Packet>,
    pub(crate) addr: SocketAddr,
    pub(crate) task: tokio::task::JoinHandle<()>,
}

impl SocketHandle {
    fn new<C: Codec>(codec: C, addr: SocketAddr) -> Result<Self, C::Error> {
        let sckt = Socket { codec };
        let (tx_w, rx_w) = mpsc::channel(16);
        let (tx_r, rx_r) = mpsc::channel(16);
        let task = tokio::spawn(async move {
            sckt.handle(tx_r, rx_w).await;
        });

        Ok(SocketHandle {
            tx_send: tx_w,
            rx_recv: rx_r,
            addr,
            task,
        })
    }

    pub async fn new_client<C: Codec>(mut io: C::IO) -> Result<Self, C::Error> {
        let addr = C::sock_addr(&mut io)?;
        let codec = C::create_client(io).await?;
        Self::new(codec, addr)
    }

    pub async fn new_server<C: Codec>(mut io: C::IO) -> Result<Self, C::Error> {
        let addr = C::sock_addr(&mut io)?;
        let codec = C::create_server(io).await?;
        Self::new(codec, addr)
    }

    pub fn try_recv(&mut self) -> Result<Packet, mpsc::error::TryRecvError> {
        self.rx_recv.try_recv()
    }

    pub async fn recv(&mut self) -> Option<Packet> {
        self.rx_recv.recv().await
    }

    pub fn send(&mut self, pkt: Packet) -> Result<(), mpsc::error::TrySendError<Packet>> {
        self.tx_send.try_send(pkt)
    }

    pub fn send_encode(
        &mut self,
        pkt: impl EncodePacket,
    ) -> Result<(), mpsc::error::TrySendError<Packet>> {
        self.tx_send.try_send(pkt.to_packet().unwrap())
    }
}

impl Drop for SocketHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[cfg(test)]
mod tests {
    use bytes::Buf;
    use turmoil::Builder;

    use crate::{
        net::EncodePacket,
        util::test_util::{bind, connect, MockCodec, MockMsgReq},
    };

    use super::SocketHandle;

    #[test]
    fn echo() {
        let mut sim = Builder::new().build();

        sim.host("server", || async {
            let listener = bind().await?;
            loop {
                let io = listener.accept().await?;
                let mut socket = SocketHandle::new_server::<MockCodec>(io.0).await?;
                while let Some(p) = socket.recv().await {
                    socket.send(p)?;
                }
                drop(socket)
            }
        });

        sim.client("client", async {
            let conn = connect().await?;
            let mut socket = SocketHandle::new_client::<MockCodec>(conn).await?;
            let mut acc = 0;
            let range = 0..10;
            for i in range.clone() {
                socket.send(MockMsgReq(i).to_packet().unwrap())?;
                let mut p = socket.rx_recv.recv().await.unwrap();
                acc += p.0.get_u32();
            }
            assert_eq!(acc, range.sum());
            Ok(())
        });

        sim.run().unwrap();
    }
}
