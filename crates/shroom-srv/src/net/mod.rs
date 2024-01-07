use bytes::{Bytes, BytesMut};
use futures::{Future, Sink, Stream};
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug, Clone)]
pub struct Packet(pub Bytes);

pub trait EncodePacket {
    fn encode(&self, b: &mut BytesMut) -> anyhow::Result<()>;

    fn to_packet(&self) -> anyhow::Result<Packet> {
        let mut buf = BytesMut::with_capacity(1024);
        self.encode(&mut buf)?;
        Ok(Packet(buf.freeze()))
    }
}

pub trait Codec: Sized + Send + 'static {
    type Error: std::fmt::Debug + From<std::io::Error> + Send + Sync + 'static;
    type Rx: Stream<Item = Result<Packet, Self::Error>> + Send + Unpin + 'static;
    type Tx: Sink<Packet> + Send + Unpin + 'static;
    type IO: AsyncRead + AsyncWrite + Unpin + Send + 'static;

    fn sock_addr(io: &mut Self::IO) -> Result<std::net::SocketAddr, Self::Error>;
    fn create_client(io: Self::IO) -> impl Future<Output = Result<Self, Self::Error>> + Send;
    fn create_server(io: Self::IO) -> impl Future<Output = Result<Self, Self::Error>> + Send;

    fn split(self) -> (Self::Rx, Self::Tx);
}

pub mod room;
pub mod session;
pub mod session_stream;
pub mod socket;
