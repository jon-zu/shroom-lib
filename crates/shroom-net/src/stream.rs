use std::ops::Deref;

use crate::{
    codec::{ShroomCodec, ShroomTransport},
    NetError, NetResult,
};

use futures::{SinkExt, StreamExt};

use shroom_pkt::Packet;
use tokio_util::codec::{FramedRead, FramedWrite};

/// Write half of a `ShroomConn` implements futures::Sink
pub struct ShroomStreamWrite<C: ShroomCodec>(
    FramedWrite<<C::Transport as ShroomTransport>::WriteHalf, C::Encoder>,
);

impl<C: ShroomCodec, T: Deref<Target = [u8]>> futures::Sink<T> for ShroomStreamWrite<C> {
    type Error = NetError;

    fn poll_ready(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.0.poll_ready_unpin(cx)
    }

    fn start_send(mut self: std::pin::Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        self.0.start_send_unpin(item.deref())
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.0.poll_flush_unpin(cx)
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.0.poll_close_unpin(cx)
    }
}

/// Read half of a `ShroomConn` implements futures::Stream
pub struct ShroomStreamRead<C: ShroomCodec>(
    FramedRead<<C::Transport as ShroomTransport>::ReadHalf, C::Decoder>,
);

impl<C: ShroomCodec> futures::Stream for ShroomStreamRead<C> {
    type Item = NetResult<Packet>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0.poll_next_unpin(cx)
    }
}

/// Shroom stream which allows to send and recv packets
pub struct ShroomStream<C: ShroomCodec> {
    r: ShroomStreamRead<C>,
    w: ShroomStreamWrite<C>,
}

impl<C: ShroomCodec, T: Deref<Target = [u8]>> futures::Sink<T> for ShroomStream<C> {
    type Error = NetError;

    fn poll_ready(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        <ShroomStreamWrite<C> as SinkExt<T>>::poll_ready_unpin(&mut self.w, cx)
    }

    fn start_send(mut self: std::pin::Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        self.w.start_send_unpin(item.deref())
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        <ShroomStreamWrite<C> as SinkExt<T>>::poll_flush_unpin(&mut self.w, cx)
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        <ShroomStreamWrite<C> as SinkExt<T>>::poll_close_unpin(&mut self.w, cx)
    }
}

impl<C: ShroomCodec> futures::Stream for ShroomStream<C> {
    type Item = NetResult<Packet>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.r.poll_next_unpin(cx)
    }
}

impl<C: ShroomCodec> std::fmt::Debug for ShroomStream<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShroomStream").finish()
    }
}

impl<C> ShroomStream<C>
where
    C: ShroomCodec + Unpin,
{
    /// Create a new session from the `io` and
    pub fn new(io: C::Transport, (enc, dec): (C::Encoder, C::Decoder)) -> Self {
        let (r, w) = io.split();
        Self {
            r: ShroomStreamRead(FramedRead::new(r, dec)),
            w: ShroomStreamWrite(FramedWrite::new(w, enc)),
        }
    }

    /// Splits the stream into write and read half references
    pub fn split(&mut self) -> (&mut ShroomStreamWrite<C>, &mut ShroomStreamRead<C>) {
        (&mut self.w, &mut self.r)
    }

    /// Splits the stream into owned write and read halves
    pub fn into_split(self) -> (ShroomStreamWrite<C>, ShroomStreamRead<C>) {
        (self.w, self.r)
    }

    /// Returns the remote address of the underlying socket
    pub async fn close(mut self) -> NetResult<()> {
        self.w.0.close().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use futures::{SinkExt, StreamExt};
    use shroom_crypto::SharedCryptoContext;
    use std::{
        net::{IpAddr, Ipv4Addr},
        ops::Deref,
        sync::Arc,
    };
    use turmoil::net::{TcpListener, TcpStream};

    use crate::codec::{
        legacy::{handshake_gen::BasicHandshakeGenerator, LegacyCodec},
        ShroomCodec,
    };

    const PORT: u16 = 1738;

    async fn bind() -> std::result::Result<TcpListener, std::io::Error> {
        TcpListener::bind((IpAddr::from(Ipv4Addr::UNSPECIFIED), PORT)).await
    }

    #[test]
    fn echo() -> anyhow::Result<()> {
        const ECHO_DATA: [&'static [u8]; 5] = [&[], &[0xFF; 4096], &[], &[1, 2], &[0x0; 1024]];

        let legacy = Arc::new(LegacyCodec::<turmoil::net::TcpStream>::new(
            SharedCryptoContext::default(),
            BasicHandshakeGenerator::v83(),
        ));

        let mut sim = turmoil::Builder::new().build();

        sim.host("server", || async move {
            let listener = bind().await?;

            let legacy = LegacyCodec::<turmoil::net::TcpStream>::new(
                SharedCryptoContext::default(),
                BasicHandshakeGenerator::v83(),
            );
            loop {
                let socket = listener.accept().await.unwrap().0;
                let mut sess = legacy.create_server(socket).await?;
                // Echo
                while let Ok(pkt) = sess.next().await.unwrap() {
                    //dbg!(pkt.len());
                    sess.send(pkt).await.unwrap();
                }
            }
        });

        sim.client("client", async move {
            let socket = TcpStream::connect(("server", PORT)).await.unwrap();
            let mut sess = legacy.create_client(socket).await.unwrap();
            for (i, data) in ECHO_DATA.iter().enumerate() {
                sess.send(Bytes::from_static(*data)).await.unwrap();
                let pkt = sess.next().await.unwrap().unwrap();
                assert_eq!(pkt.deref(), *data, "failed at: {i}");
            }

            Ok(())
        });

        sim.run().unwrap();

        Ok(())
    }
}
