use thiserror::Error;

#[derive(Debug, Error)]
pub enum QueueError {
    #[error("queue is full")]
    QueueFull,
}

#[derive(Debug, Error)]
pub enum EventError {
    #[error("missing required field: {0}")]
    MissingField(&'static str),
}
