#![allow(missing_docs)]

use std::fmt;

/// An error describing appliance failures.
pub enum Error<M> {
    /// Indicates that a message was not sent because of the
    /// appliance's buffer being full.
    FullBuffer(M),
    /// Indicates that a timeout of the operation has been
    /// reached.
    Timeout,
    /// Indicates that an error at this point is unexpected.
    UnexpectedFailure(Option<M>),
}

impl<M> fmt::Debug for Error<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Timeout => write!(f, "Timeout"),
            Error::FullBuffer(..) => write!(f, "FullBuffer(..)"),
            Error::UnexpectedFailure(..) => write!(f, "UnexpectedFailure(..)"),
        }
    }
}

impl<M> fmt::Display for Error<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Timeout => write!(f, "send timeout is exceeded"),
            Error::FullBuffer(..) => write!(f, "appliance buffer is full"),
            Error::UnexpectedFailure(..) => write!(f, "unexpected failure"),
        }
    }
}

impl<M> std::error::Error for Error<M> {}
