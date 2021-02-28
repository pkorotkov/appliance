#![allow(missing_docs)]

use std::fmt;

use flume::TrySendError;

/// An error describing appliance failures.
#[derive(Debug)]
pub enum Error {
    /// Indicates that a message was not sent because of the
    /// appliance's buffer being full.
    FullBuffer,
    /// Indicates that a timeout of the operation has been
    /// reached.
    Timeout,
    /// Indicates that an error at this point is unexpected.
    UnexpectedFailure,
    /// Indicates that the appliance's handling loop is stopped.
    /// This is a fatal failure which signals that the appliance
    /// is irreparable and should not be further used.
    Stopped,
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::FullBuffer => write!(f, "appliance buffer is full"),
            Error::Timeout => write!(f, "send timeout is exceeded"),
            Error::UnexpectedFailure => write!(f, "unexpected failure"),
            Error::Stopped => write!(f, "appliance is stopped"),
        }
    }
}

impl<T> From<TrySendError<T>> for Error {
    fn from(err: TrySendError<T>) -> Self {
        match err {
            TrySendError::Full(_) => Error::FullBuffer,
            TrySendError::Disconnected(_) => Error::Stopped,
        }
    }
}
