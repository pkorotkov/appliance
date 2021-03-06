#![allow(missing_docs)]

use std::fmt;

/// An error describing appliance failures.
pub enum Error {
    /// Indicates that a message was not sent because of the
    /// appliance's buffer being full.
    FullBuffer,
    /// Indicates that a timeout of the operation has been
    /// reached.
    Timeout,
    /// Indicates that an error at this point is unexpected.
    UnexpectedFailure,
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::FullBuffer => write!(f, "FullBuffer"),
            Error::Timeout => write!(f, "Timeout"),
            Error::UnexpectedFailure => write!(f, "UnexpectedFailure"),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::FullBuffer => write!(f, "appliance buffer is full"),
            Error::Timeout => write!(f, "send timeout is exceeded"),
            Error::UnexpectedFailure => write!(f, "unexpected failure"),
        }
    }
}

impl std::error::Error for Error {}
