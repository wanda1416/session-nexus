use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::models::{CreateSessionOptions, Session};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug)]
pub struct ConnectorError {
    pub message: String,
}

impl ConnectorError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ConnectorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ConnectorError {}

/// Control surface for one kind of agent.
///
/// Phase 1 implements this for CLI providers. `send_message` uses the
/// provider's non-interactive command. PTY attachment is a later method.
pub trait Connector: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;

    fn discover(&self) -> BoxFuture<'_, Result<Vec<Session>, ConnectorError>>;
    fn read_state<'a>(
        &'a self,
        session: &'a Session,
    ) -> BoxFuture<'a, Result<Session, ConnectorError>>;
    fn send_message<'a>(
        &'a self,
        session: &'a Session,
        message: &'a str,
    ) -> BoxFuture<'a, Result<(), ConnectorError>>;
    fn wait_for_completion<'a>(
        &'a self,
        session: &'a Session,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<Session, ConnectorError>>;
    fn create_session<'a>(
        &'a self,
        opts: &'a CreateSessionOptions,
    ) -> BoxFuture<'a, Result<Session, ConnectorError>>;
}
