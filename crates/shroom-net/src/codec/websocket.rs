use std::task::Poll;

use bytes::{Bytes, BytesMut};
use futures::{
    stream::{SplitSink, SplitStream},
    Sink, SinkExt, Stream, StreamExt,
};
use shroom_pkt::Packet;
use tokio::{io::AsyncRead, io::AsyncWrite};
use tokio_websockets::WebSocketStream;

use crate::NetError;

use super::{ShroomCodec, ShroomTransport};

pub struct WebSocketCodec<T> {
    uri: http::Uri,
    _marker: std::marker::PhantomData<T>,
}

impl<T> WebSocketCodec<T> {
    pub fn new(uri: http::Uri) -> Self {
        Self {
            uri,
            _marker: std::marker::PhantomData,
        }
    }
}

pub struct WsSink<T> {
    sink: SplitSink<WebSocketStream<T>, tokio_websockets::Message>,
    buf: BytesMut

}

impl<T> WsSink<T> {
    pub fn new(sink: SplitSink<WebSocketStream<T>, tokio_websockets::Message>) -> Self {
        Self {
            sink,
            buf: BytesMut::new(),
        }
    }

    pub fn create_msg(&mut self, item: &[u8]) -> tokio_websockets::Message {
        // TODO there must be a better way to avoid double buffering
        // the sink should work on byte buffers
        self.buf.reserve(item.len());
        self.buf.extend_from_slice(item);
        tokio_websockets::Message::binary(self.buf.split().freeze())
    }
}

impl<'a, T: AsyncRead + AsyncWrite + Unpin> Sink<&'a [u8]> for WsSink<T> {
    type Error = NetError;

    fn poll_ready(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.sink.poll_ready_unpin(cx).map_err(|err| err.into())
    }

    fn start_send(mut self: std::pin::Pin<&mut Self>, item: &'a [u8]) -> Result<(), Self::Error> {
        //TODO remove alloc
        let msg = self.create_msg(item);
        self.sink
            .start_send_unpin(msg)
            .map_err(|err| err.into())
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.sink.poll_flush_unpin(cx).map_err(|err| err.into())
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.sink.poll_close_unpin(cx).map_err(|err| err.into())
    }
}

pub struct WsStream<T>(SplitStream<WebSocketStream<T>>);

impl<T: AsyncRead + AsyncWrite + Unpin> Stream for WsStream<T> {
    type Item = Result<Packet, NetError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match self.0.poll_next_unpin(cx) {
            Poll::Ready(Some(res)) => Poll::Ready(Some(res.map_err(|err| err.into()).map(|msg| {
                let data: Bytes = msg.into_payload().into();
                Packet::from(data)
            }))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<T: ShroomTransport + Sync> ShroomCodec for WebSocketCodec<T> {
    type Stream = WsStream<Self::Transport>;
    type Sink = WsSink<Self::Transport>;
    type Transport = T;

    async fn create_client(
        &self,
        trans: Self::Transport,
    ) -> crate::NetResult<crate::ShroomStream<Self>> {
        let ws = tokio_websockets::ClientBuilder::from_uri(self.uri.clone())
            .connect_on(trans)
            .await?;
        let (w, r) = ws.0.split();
        Ok(crate::ShroomStream::new(WsSink::new(w), WsStream(r)))
    }

    async fn create_server(
        &self,
        trans: Self::Transport,
    ) -> crate::NetResult<crate::ShroomStream<Self>> {
        let ws = tokio_websockets::ServerBuilder::new().accept(trans).await?;
        let (w, r) = ws.split();
        Ok(crate::ShroomStream::new(WsSink::new(w), WsStream(r)))
    }
}
