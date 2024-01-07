use dashmap::DashMap;
use futures::{Future, FutureExt};
use std::{
    hash::Hash,
    ops::{Deref, DerefMut},
    panic::AssertUnwindSafe,
    sync::Arc,
};
use thiserror::Error;
use tokio::sync::Mutex;

pub mod migration;

// TODO
// * use last resort save + handle the scenario the backend fails
// * create_claimed_session can fail to claim the session, due to first creating then claiming

/// Backend for the sessions, to load and save the session data
pub trait SessionBackend {
    /// Session Data
    type Data;
    /// Parameter for the loading the session data
    type LoadParam;
    /// Input for a session transition
    type TransitionInput;
    /// Error type
    type Error: std::fmt::Debug;

    /// Loads the session data with the given parameter
    fn load(
        &self,
        param: Self::LoadParam,
    ) -> impl Future<Output = Result<Self::Data, Self::Error>> + Send;
    /// Saves the session data
    fn save(
        &self,
        session: &mut Self::Data,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// When saving fails this function will be called
    /// a good idea is to save the data to an error file so It's not lost
    #[allow(unused_variables)]
    fn last_resort_save(&self, session: &mut Self::Data) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Closes the session
    fn close(
        &self,
        session: &mut Self::Data,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Transition the session into a new state
    fn transition(
        &self,
        session: &mut Self::Data,
        input: Self::TransitionInput,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/// Represents a session which is owned by this struct
#[derive(Debug)]
pub struct OwnedSession<Key, Data> {
    key: Key,
    session: tokio::sync::OwnedMutexGuard<Data>,
}

impl<Key, Data> OwnedSession<Key, Data> {
    /// Create a new owned session, from the locked session and the key
    pub fn new(key: Key, session: tokio::sync::OwnedMutexGuard<Data>) -> Self {
        Self { key, session }
    }

    /// Obtain the key of the owned session
    pub fn key(&self) -> &Key {
        &self.key
    }

    /// Map the session to a specific value
    pub fn map<Mapped, F>(mut self, f: F) -> OwnedMappedSession<Key, Data, Mapped>
    where
        F: FnOnce(&mut Data) -> &mut Mapped,
    {
        let mapped = f(&mut self.session) as *mut Mapped;
        OwnedMappedSession {
            session: self,
            mapped,
        }
    }

    /// Map the session to a specific value
    pub fn try_map<Mapped, F, E>(mut self, f: F) -> Result<OwnedMappedSession<Key, Data, Mapped>, E>
    where
        F: FnOnce(&mut Data) -> Result<&mut Mapped, E>,
    {
        let mapped = f(&mut self.session)? as *mut Mapped;
        Ok(OwnedMappedSession {
            session: self,
            mapped,
        })
    }
}

impl<Key, SessionData> Deref for OwnedSession<Key, SessionData> {
    type Target = SessionData;

    fn deref(&self) -> &Self::Target {
        self.session.deref()
    }
}

impl<Key, SessionData> DerefMut for OwnedSession<Key, SessionData> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.session.deref_mut()
    }
}

/// Represents a session which is owned,
/// but dereferences to the mapped value
#[derive(Debug)]
pub struct OwnedMappedSession<Key, Data, Mapped> {
    session: OwnedSession<Key, Data>,
    mapped: *mut Mapped,
}

impl<Key, Data, Mapped> OwnedMappedSession<Key, Data, Mapped> {
    /// Unmap the session, returning the owned session
    pub fn unmap(self) -> OwnedSession<Key, Data> {
        self.session
    }
}

impl<Key, Data, Mapped> Deref for OwnedMappedSession<Key, Data, Mapped> {
    type Target = Mapped;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mapped }
    }
}

impl<Key, Data, Mapped> DerefMut for OwnedMappedSession<Key, Data, Mapped> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mapped }
    }
}

/// Safety: When all types are Send so is the mapped session
unsafe impl<Key, Data, Mapped> Send for OwnedMappedSession<Key, Data, Mapped>
where
    Key: Send,
    Data: Send,
    Mapped: Send,
{
}

/// Safety: When all types are Sync so is the mapped session
unsafe impl<Key, Data, Mapped> Sync for OwnedMappedSession<Key, Data, Mapped>
where
    Key: Sync,
    Data: Sync,
    Mapped: Sync,
{
}

pub type SharedSessionHandle<Data> = Arc<tokio::sync::Mutex<Data>>;

#[derive(Debug)]
pub struct SessionManager<Key: Eq + Hash, Backend: SessionBackend> {
    sessions: DashMap<Key, SharedSessionHandle<Backend::Data>>,
    backend: Backend,
}

#[derive(Debug, Error)]
pub enum SessionError<BackendErr> {
    #[error("data store disconnected: {0:?}")]
    Backend(#[from] BackendErr),
    #[error("panic occured during saving")]
    SavePanic,
    #[error("session key already exists")]
    SessionKeyAlreadyExists,
    #[error("unable to lock session")]
    UnableToLockSession,
    #[error("Session for key does not exist")]
    SessionKeyNotExists,
}

pub type SessionResult<T, B> = Result<T, SessionError<<B as SessionBackend>::Error>>;

impl<Key, Backend> SessionManager<Key, Backend>
where
    Key: Eq + Hash + Clone,
    Backend: SessionBackend,
{
    pub fn new(backend: Backend) -> Self {
        Self {
            sessions: DashMap::new(),
            backend,
        }
    }

    pub fn session(&self) -> usize {
        self.sessions.len()
    }

    /// Helper function to close a session
    async fn close_session_inner(
        &self,
        session_data: &mut Backend::Data,
    ) -> SessionResult<(), Backend> {
        // After the session is removed save It
        self.backend
            .save(session_data)
            .await
            .map_err(SessionError::Backend)?;

        self.backend
            .close(session_data)
            .await
            .map_err(SessionError::Backend)?;

        Ok(())
    }

    /// Closes a session, but catches potential panics
    /// and errors during the process to close the session
    async fn safe_close(&self, session_data: &mut Backend::Data) -> SessionResult<(), Backend> {
        let res = AssertUnwindSafe(self.close_session_inner(session_data))
            .catch_unwind()
            .await;
        match res {
            Ok(res) => res,
            Err(_) => Err(SessionError::SavePanic),
        }
    }

    /// Create a session with the given key and data
    fn create_session_from_data(
        &self,
        key: Key,
        data: Backend::Data,
    ) -> SessionResult<(), Backend> {
        let mut inserted = false;
        self.sessions.entry(key).or_insert_with(|| {
            inserted = true;
            Arc::new(Mutex::new(data))
        });

        if !inserted {
            return Err(SessionError::SessionKeyAlreadyExists);
        }

        Ok(())
    }

    /// Gets  a shared session handle by key
    fn get_session(&self, key: &Key) -> SessionResult<SharedSessionHandle<Backend::Data>, Backend> {
        Ok(self
            .sessions
            .get(key)
            .ok_or_else(|| SessionError::SessionKeyNotExists)?
            .value()
            .clone())
    }

    /// Transition a session into a new state, using the transition input
    pub async fn transition(
        &self,
        session: &mut OwnedSession<Key, Backend::Data>,
        input: Backend::TransitionInput,
    ) -> SessionResult<(), Backend> {
        self.backend
            .transition(session, input)
            .await
            .map_err(SessionError::Backend)?;

        Ok(())
    }

    /// Remove all un-owned session
    /// acts essentially like a life-cycle
    pub async fn remove_unowned_session(&self) -> anyhow::Result<()> {
        let mut removed = vec![];

        // Drain all un-owned sessions
        self.sessions.retain(|_, v| {
            if let Ok(lock) = v.clone().try_lock_owned() {
                removed.push((lock, v.clone()));
                false
            } else {
                true
            }
        });

        // As we are the last holder of the session we also need to close them
        for (session_lock, session) in removed.into_iter() {
            // Drop the lock
            drop(session_lock);
            // We held the lock before removing it, so there's only this reference left
            //let session = Arc::try_unwrap(session).unwrap_or_else(|_| panic!("locked session")));
            let mut session = session.try_lock().unwrap();

            let res = self.safe_close(&mut session).await;
            if let Err(err) = res {
                log::error!("Error during saving Session: {err:?}");
            }
        }

        Ok(())
    }

    /// Closes an owned session
    pub async fn close_session(
        &self,
        owned_session: OwnedSession<Key, Backend::Data>,
    ) -> SessionResult<(), Backend> {
        // Remove session, If the session exist It must be in the map
        let (_, session) = self.sessions.remove(&owned_session.key).expect("Session");
        // Release lock to decrement the ref count to 1
        drop(owned_session);
        // Now we can claim the session
        let mut session = session.try_lock().unwrap();
        self.safe_close(&mut session).await
    }

    /// Create a sessions with the given key and the load parameter
    /// the session data will be fetched with the backend
    pub async fn create_session(
        &self,
        key: Key,
        param: Backend::LoadParam,
    ) -> SessionResult<(), Backend> {
        let data = self
            .backend
            .load(param)
            .await
            .map_err(SessionError::Backend)?;
        self.create_session_from_data(key, data)
    }

    /// Creates a claimed session, like `create_session`
    /// but this will also claim the session after create the session
    pub async fn create_claimed_session(
        &self,
        key: Key,
        param: Backend::LoadParam,
    ) -> SessionResult<OwnedSession<Key, Backend::Data>, Backend> {
        self.create_session(key.clone(), param).await?;
        // There can be atmost one session per key
        // so we can assume we can always claim that session
        Ok(self
            .try_claim_session(key)
            .unwrap_or_else(|_| panic!("claim after create")))
    }

    /// Tries to claim a session for the given key
    pub fn try_claim_session(
        &self,
        key: Key,
    ) -> SessionResult<OwnedSession<Key, Backend::Data>, Backend> {
        let session = self.get_session(&key)?;

        Ok(OwnedSession::new(
            key,
            session
                .try_lock_owned()
                .map_err(|_| SessionError::UnableToLockSession)?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub struct MockSessionBackend;

    impl SessionBackend for MockSessionBackend {
        type Data = usize;
        type LoadParam = usize;
        type TransitionInput = usize;
        type Error = anyhow::Error;

        async fn load(&self, param: Self::LoadParam) -> Result<Self::Data, Self::Error> {
            Ok(param)
        }
        async fn save(&self, _session: &mut Self::Data) -> Result<(), Self::Error> {
            Ok(())
        }

        /// Closes the session
        async fn close(&self, _session: &mut Self::Data) -> Result<(), Self::Error> {
            Ok(())
        }

        /// Transition the session into a new state
        async fn transition(
            &self,
            session: &mut Self::Data,
            input: Self::TransitionInput,
        ) -> Result<(), Self::Error> {
            *session += input;
            Ok(())
        }
    }

    #[tokio::test]
    async fn session_man() {
        let sm = SessionManager::<u32, MockSessionBackend>::new(MockSessionBackend);

        let mut sess = sm.create_claimed_session(1, 0).await.unwrap();
        assert_eq!(sess.deref(), &0);
        assert_eq!(sess.key, 1);
        assert!(sm.try_claim_session(1).is_err());

        sm.transition(&mut sess, 10).await.unwrap();
        assert_eq!(sess.deref(), &10);

        // No sessions should be removed
        sm.remove_unowned_session().await.unwrap();
        assert_eq!(sm.session(), 1);
        // Drop our session handle
        drop(sess);
        assert_eq!(sm.session(), 1);

        // Now the sessions should be removed
        sm.remove_unowned_session().await.unwrap();
        assert_eq!(sm.session(), 0);

        // Create a new session and close it properly
        let sess = sm.create_claimed_session(1, 0).await.unwrap();
        assert_eq!(sm.session(), 1);
        sm.close_session(sess).await.unwrap();
        assert_eq!(sm.session(), 0);
    }
}
