#![allow(non_upper_case_globals)]

pub mod legacy;

use std::pin::Pin;

use futures::Future;
use shroom_pkt::Packet;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{NetError, NetResult, ShroomStream};

use tokio_util::codec::{Decoder, Encoder};

pub trait ShroomTransport: AsyncWrite + AsyncRead + Unpin + Send + 'static {
    type ReadHalf: AsyncRead + Unpin + Send + 'static;
    type WriteHalf: AsyncWrite + Unpin + Send + 'static;

    fn peer_addr(&self) -> NetResult<std::net::SocketAddr>;
    fn local_addr(&self) -> NetResult<std::net::SocketAddr>;

    fn split(self) -> (Self::ReadHalf, Self::WriteHalf);
}

pub struct LocalShroomTransport<T>(pub T);

impl<T> ShroomTransport for LocalShroomTransport<T>
where
    T: AsyncWrite + AsyncRead + Unpin + Send + 'static,
{
    type ReadHalf = tokio::io::ReadHalf<T>;
    type WriteHalf = tokio::io::WriteHalf<T>;

    fn peer_addr(&self) -> NetResult<std::net::SocketAddr> {
        Ok(std::net::SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
            0,
        ))
    }

    fn local_addr(&self) -> NetResult<std::net::SocketAddr> {
        Ok(std::net::SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
            0,
        ))
    }

    fn split(self) -> (Self::ReadHalf, Self::WriteHalf) {
        tokio::io::split(self.0)
    }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for LocalShroomTransport<T> {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        let this = self.get_mut();
        Pin::new(&mut this.0).poll_write(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();
        Pin::new(&mut this.0).poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();
        Pin::new(&mut this.0).poll_shutdown(cx)
    }
}

impl<T: AsyncRead + Unpin> AsyncRead for LocalShroomTransport<T> {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        Pin::new(&mut this.0).poll_read(cx, buf)
    }
}

impl ShroomTransport for tokio::net::TcpStream {
    type ReadHalf = tokio::net::tcp::OwnedReadHalf;
    type WriteHalf = tokio::net::tcp::OwnedWriteHalf;

    fn peer_addr(&self) -> NetResult<std::net::SocketAddr> {
        self.peer_addr().map_err(|e| e.into())
    }

    fn local_addr(&self) -> NetResult<std::net::SocketAddr> {
        self.local_addr().map_err(|e| e.into())
    }

    fn split(self) -> (Self::ReadHalf, Self::WriteHalf) {
        self.into_split()
    }
}

#[cfg(test)]
impl ShroomTransport for turmoil::net::TcpStream {
    type ReadHalf = turmoil::net::tcp::OwnedReadHalf;
    type WriteHalf = turmoil::net::tcp::OwnedWriteHalf;

    fn peer_addr(&self) -> NetResult<std::net::SocketAddr> {
        self.peer_addr().map_err(|e| e.into())
    }

    fn local_addr(&self) -> NetResult<std::net::SocketAddr> {
        self.local_addr().map_err(|e| e.into())
    }

    fn split(self) -> (Self::ReadHalf, Self::WriteHalf) {
        self.into_split()
    }
}

/// Codec trait
pub trait ShroomCodec: Sized + Unpin + Send + Sync {
    type Encoder:  for<'a> Encoder<&'a[u8], Error = NetError> + Send + 'static;
    type Decoder: Decoder<Item = Packet, Error = NetError> + Send + 'static;
    type Transport: ShroomTransport;

    fn create_client(
        &self,
        trans: Self::Transport,
    ) -> impl Future<Output = NetResult<ShroomStream<Self>>> + Send;
    fn create_server(
        &self,
        trans: Self::Transport,
    ) -> impl Future<Output = NetResult<ShroomStream<Self>>> + Send;
}
