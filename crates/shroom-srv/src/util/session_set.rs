use tokio::sync::mpsc;

use crate::net::{
    session::{Session, SessionHandler},
    Packet,
};

pub struct SessionEntry<H: SessionHandler> {
    pub error: bool,
    pub session: Session<H>,
    pub tx: mpsc::Sender<Packet>,
}

pub struct SessionSet<H: SessionHandler>(Vec<SessionEntry<H>>);

impl<H: SessionHandler> Default for SessionSet<H> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<H: SessionHandler> SessionSet<H> {
    pub fn broadcast_pkt(&mut self, pkt: Packet) {
        for sess in self.0.iter_mut() {
            if sess.error {
                continue;
            }

            if let Err(err) = sess.tx.try_send(pkt.clone()) {
                sess.error = true;
            }
        }
    }

    pub fn add(&mut self, session: Session<H>) {
        let session_tx = session.handler.session().socket_handle.tx_w.clone();
        self.0.push(SessionEntry {
            error: false,
            session,
            tx: session_tx,
        })
    }
}
