use thiserror::Error;

#[derive(Debug, Error)]
pub enum QueueError {
    #[error("queue is full")]
    QueueFull,
}

#[derive(Debug, Error)]
pub enum EventError {
    // unconstructable until v2 adds event variants; the variant exists now
    // so the §11 error→status table is complete from day one
    #[allow(dead_code)]
    #[error("unknown event type: {0}")]
    UnknownType(String),
    #[error("missing required field: {0}")]
    MissingField(&'static str),
}
