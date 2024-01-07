use std::sync::Arc;

use futures::{Future, Stream, StreamExt};
use tokio::sync::mpsc;

use super::{
    session::{Session, SessionHandler},
    socket::SocketHandle,
    Codec,
};

pub trait SessionStreamHandler: Sized {
    type Error: std::fmt::Debug;
    type Codec: Codec<Error = Self::Error> + Send + 'static;
    type SessionHandler: SessionHandler + Send + 'static;

    fn create_session(
        &self,
        socket: &mut SocketHandle,
    ) -> impl Future<Output = Result<Self::SessionHandler, Self::Error>> + Send;
}

pub struct SessionStreamHandle<H: SessionStreamHandler> {
    rx: mpsc::Receiver<Session<H::SessionHandler>>,
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
}

impl<H: SessionStreamHandler> Drop for SessionStreamHandle<H> {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl<H: SessionStreamHandler> Stream for SessionStreamHandle<H> {
    type Item = Session<H::SessionHandler>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}

pub struct SessionStream<H: SessionStreamHandler, S> {
    handler: Arc<H>,
    stream: S,
}

impl<H: SessionStreamHandler, S> SessionStream<H, S>
where
    H: SessionHandler + Send + Sync + 'static,
    S: Stream<Item = <H::Codec as Codec>::IO> + Send + 'static + Unpin,
{
    pub fn new(handler: H, stream: S) -> Self {
        Self {
            handler: Arc::new(handler),
            stream,
        }
    }

    async fn handle_session(
        ctx: Arc<H>,
        io: <H::Codec as Codec>::IO,
        session_tx: mpsc::Sender<Session<H::SessionHandler>>,
    ) -> Result<(), H::Error> {
        let mut sck = SocketHandle::new_server::<H::Codec>(io).await?;
        let handler = ctx.create_session(&mut sck).await?;
        let session = Session::new(handler, sck).await.unwrap(); //TODO
        session_tx.send(session).await.unwrap(); //TODO
        Ok(())
    }

    /// Spawns the listener loop and returns a handle
    pub async fn spawn(mut self) -> SessionStreamHandle<H> {
        let (session_tx, session_rx) = mpsc::channel(16);
        let task = tokio::spawn(async move {
            while let Some(io) = self.stream.next().await {
                let tx = session_tx.clone();
                let handler = self.handler.clone();
                tokio::spawn(async move {
                    if let Err(err) = Self::handle_session(handler, io, tx).await {
                        log::error!("Session error: {:?}", err);
                    }
                });
            }

            Ok(())
        });

        SessionStreamHandle {
            rx: session_rx,
            task,
        }
    }
}
