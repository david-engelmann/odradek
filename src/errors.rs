//! Error types for zombie-hunt.
//!
//! The first four variants are the refusal classes the Python
//! tool raised, one per surface that can refuse, and each
//! carries the fully formed message the command line prints
//! after its error prefix. The last variant is the uncaught
//! family, the failures the Python tool never caught, which
//! the Python interpreter answered with a traceback at exit
//! status 1. The crate answers those with its own one line at
//! the same status, so the variant carries the line to print,
//! formed by the port rather than copied from any traceback.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// A recorded cluster snapshot is malformed.
    #[error("{0}")]
    Snapshot(String),
    /// A quarantine plan cannot be written for the named entry.
    #[error("{0}")]
    Plan(String),
    /// An observation report cannot be written for the named entry.
    #[error("{0}")]
    Observe(String),
    /// A recorded kill call fixture is outside the recorded shape.
    #[error("{0}")]
    Judgment(String),
    /// A failure the Python tool never caught.
    #[error("{0}")]
    Uncaught(String),
}

impl Error {
    /// The message the error carries.
    pub fn message(&self) -> &str {
        match self {
            Error::Snapshot(message)
            | Error::Plan(message)
            | Error::Observe(message)
            | Error::Judgment(message)
            | Error::Uncaught(message) => message,
        }
    }

    /// Whether this error belongs to the uncaught family.
    pub fn is_uncaught(&self) -> bool {
        matches!(self, Error::Uncaught(_))
    }
}

/// The result type every fallible library function returns.
pub type Result<T> = std::result::Result<T, Error>;
