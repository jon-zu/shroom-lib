use std::net::IpAddr;

use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use shroom_net::{
    codec::{ShroomCodec, ShroomTransport},
    stream::{ShroomStreamRead, ShroomStreamWrite},
    NetError, ShroomStream,
};
use shroom_pkt::{util::packet_buf::PacketBuf, Packet};
use tokio::sync::mpsc;

use super::room_set::PktMsg;

pub type PacketMsgTx = mpsc::Sender<PktMsg>;
pub type PacketMsgRx = mpsc::Receiver<PktMsg>;

pub type PacketTx = mpsc::Sender<Packet>;
pub type PacketRx = mpsc::Receiver<Packet>;

#[derive(Debug)]
pub struct ServerSocket<C: ShroomCodec>(ShroomStream<C>);

impl<C: ShroomCodec> ServerSocket<C> {
    pub async fn handle(self, tx: PacketTx, rx: PacketMsgRx) {
        let (mut w, mut r) = self.0.into_split();
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

    async fn write(rx: &mut PacketMsgRx, w: &mut ShroomStreamWrite<C>) -> Result<(), NetError> {
        while let Some(msg) = rx.recv().await {
            match msg {
                PktMsg::Packet(pkt) => {
                    if let Err(err) = w.send(pkt).await {
                        log::error!("tx error: {:?}", err);
                        return Err(err);
                    }
                }
                PktMsg::PacketBuf(buf) => {
                    for pkt in buf.packets() {
                        //TODO
                        if let Err(err) = w.send(Bytes::from(pkt.to_vec())).await {
                            log::error!("tx error: {:?}", err);
                            return Err(err);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn read(tx: &mut PacketTx, r: &mut ShroomStreamRead<C>) -> Result<(), NetError> {
        while let Some(msg) = r.next().await {
            // TODO handle out of space and tx dropped
            let msg = msg?;
            if let Err(err) = tx.send(msg).await {
                log::error!("rx error: {:?}", err);
                break;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ServerSocketHandle {
    pub(crate) tx_send: PacketMsgTx,
    pub(crate) rx_recv: PacketRx,
    pub(crate) task: tokio::task::JoinHandle<()>,
    peer_addr: IpAddr,
}

impl ServerSocketHandle {
    fn new<C: ShroomCodec + 'static>(
        conn: ShroomStream<C>,
        peer_addr: IpAddr,
    ) -> Result<Self, NetError> {
        let (tx_w, rx_w) = mpsc::channel(16);
        let (tx_r, rx_r) = mpsc::channel(16);

        let conn = ServerSocket(conn);
        let task = tokio::spawn(async move {
            conn.handle(tx_r, rx_w).await;
        });

        Ok(Self {
            tx_send: tx_w,
            rx_recv: rx_r,
            peer_addr,
            task,
        })
    }

    pub async fn new_client<C: ShroomCodec + 'static>(
        codec: &C,
        io: C::Transport,
    ) -> Result<Self, NetError> {
        let peer_addr = io.peer_addr()?.ip();
        let codec = codec.create_client(io).await?;
        Self::new(codec, peer_addr)
    }

    pub async fn new_server<C: ShroomCodec + 'static>(
        codec: &C,
        io: C::Transport,
    ) -> Result<Self, NetError> {
        let peer_addr = io.peer_addr()?.ip();
        let codec = codec.create_server(io).await?;
        Self::new(codec, peer_addr)
    }

    pub fn try_recv(&mut self) -> Result<Packet, mpsc::error::TryRecvError> {
        self.rx_recv.try_recv()
    }

    pub async fn recv(&mut self) -> Option<Packet> {
        self.rx_recv.recv().await
    }

    pub fn send(&mut self, pkt: Packet) -> Result<(), mpsc::error::TrySendError<PktMsg>> {
        self.tx_send.try_send(pkt.into())
    }

    pub fn send_buf(&mut self, pkt: PacketBuf) -> Result<(), mpsc::error::TrySendError<PktMsg>> {
        self.tx_send.try_send(pkt.into())
    }

    pub fn peer_addr(&self) -> IpAddr {
        self.peer_addr
    }
}

impl Drop for ServerSocketHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}
