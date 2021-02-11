//! A lightweight actor model inspired framework to build
//! customizable componets with message-based intercommunications.

#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]

use std::{any::Any, error, fmt, panic::catch_unwind, thread, time::Duration};
use {
    async_io::block_on,
    flume::{bounded, unbounded, RecvError, RecvTimeoutError, Sender, TrySendError},
    futures_lite::future::pending,
    once_cell::sync::Lazy,
};

/// An error describing appliance failures.
#[derive(Debug)]
pub enum Error {
    /// Indicates that a message was not sent because of the
    /// appliance's buffer being full.
    FullBuffer,
    /// Indicates that a timeout of the operation has been
    /// reached.
    Timeout,
    /// Indicates that a message sent can not be handled by
    /// the appliance and ignored.
    UnhandledMessage,
    /// Indicates that the appliance's handling loop is stopped.
    /// This is a fatal failure which signals that the appliance
    /// is irreparable and should not be further used.
    Stopped,
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::FullBuffer => write!(f, "appliance buffer is full"),
            Error::Timeout => write!(f, "send timeout is exeeded"),
            Error::UnhandledMessage => write!(f, "sent message left unhandled"),
            Error::Stopped => write!(f, "appliance is stopped"),
        }
    }
}

/// An async executor.
pub type Executor<'a> = async_executor::Executor<'a>;

/// A default executor. It runs on per-core threads and is fair
/// in terms of task priorities.
pub static DEFAULT_EXECUTOR: Lazy<Executor<'_>> = Lazy::new(|| {
    let num_threads = num_cpus::get();
    for n in 1..=num_threads {
        thread::Builder::new()
            .name(format!("appliance-{}", n))
            .spawn(|| loop {
                catch_unwind(|| block_on(DEFAULT_EXECUTOR.run(pending::<()>()))).ok();
            })
            .expect("cannot spawn an appliance executor thread");
    }
    Executor::new()
});

/// Internal marker of fire-and-forget sending.
enum Marker {}

/// A stateful entity that only allows to
/// interact with via handling messages.
#[derive(Debug)]
pub struct Appliance {
    in_: Sender<Box<dyn Any + Send>>,
}

impl<'a> Appliance {
    /// Creates a new appliance with a bounded message buffer,
    /// a state, and a handler.
    pub fn new_bounded<S: Send + 'a, M: Send + 'static, R: Send + 'static>(
        executor: &Executor<'a>,
        state: S,
        handler: impl Fn(&mut S, M) -> R + Send + 'a,
        size: usize,
    ) -> Self {
        Self::new(executor, state, handler, Some(size))
    }

    /// Creates a new appliance with an unbounded message buffer,
    /// a state, and a handler.
    pub fn new_unbounded<S: Send + 'a, M: Send + 'static, R: Send + 'static>(
        executor: &Executor<'a>,
        state: S,
        handler: impl Fn(&mut S, M) -> R + Send + 'a,
    ) -> Self {
        Self::new(executor, state, handler, None)
    }

    /// Sends a message to the current appliance without
    /// waiting for the message to be handled.
    pub fn send<M: Send + 'static>(&self, message: M) -> Result<(), Error> {
        let m: Box<(M, Option<Sender<Marker>>)> = Box::new((message, None));
        to_result(self.in_.try_send(m))
    }

    /// Sends a message to the current appliance and waits
    /// indefinitely for the message to be handled.
    /// This blocking method is a fit for callers who
    /// must be assured that the message has been handled.
    /// It is supposed to be used less often then `send` as
    /// it may suffer a performance hit due to synchronization
    /// with the handling loop.
    pub fn send_and_wait<M: Send + 'static, R: Send + 'static>(
        &self,
        message: M,
    ) -> Result<R, Error> {
        let (s, r) = bounded(1);
        let m: Box<(M, Option<Sender<R>>)> = Box::new((message, Some(s)));
        let result = to_result(self.in_.try_send(m));
        match result {
            Err(e) => Err(e),
            _ => match r.recv() {
                Ok(r) => Ok(r),
                Err(RecvError::Disconnected) => Err(Error::UnhandledMessage),
            },
        }
    }

    /// Sends a message to the current appliance and waits
    /// for only given time (timeout) for the message to be
    /// handled. This blocking method is a fit for callers
    /// who must be assured that the message has been handled.
    /// It is supposed to be used less often then `send` as
    /// it may suffer a performance hit due to synchronization
    /// with the handling loop.
    pub fn send_and_wait_with_timeout<M: Send + 'static, R: Send + 'static>(
        &self,
        message: M,
        d: Duration,
    ) -> Result<R, Error> {
        let (s, r) = bounded(1);
        let m: Box<(M, Option<Sender<R>>)> = Box::new((message, Some(s)));
        let result = to_result(self.in_.try_send(m));
        match result {
            Err(e) => Err(e),
            _ => match r.recv_timeout(d) {
                Ok(r) => Ok(r),
                Err(RecvTimeoutError::Disconnected) => Err(Error::UnhandledMessage),
                Err(RecvTimeoutError::Timeout) => Err(Error::Timeout),
            },
        }
    }

    /// Creates a new appliance with the given state, message handler,
    /// and buffer size, if any.
    fn new<S: Send + 'a, M: Send + 'static, R: Send + 'static>(
        executor: &Executor<'a>,
        mut state: S,
        handler: impl Fn(&mut S, M) -> R + Send + 'a,
        size: Option<usize>,
    ) -> Self {
        let (in_, out_) = if let Some(mbs) = size {
            bounded(mbs)
        } else {
            unbounded()
        };
        let appliance = Appliance { in_ };
        executor
            .spawn(async move {
                loop {
                    match out_.recv_async().await {
                        Ok(o) => {
                            match o.downcast::<(M, Option<Sender<Marker>>)>() {
                                Ok(p) => {
                                    let (m, _) = *p;
                                    handler(&mut state, m);
                                }
                                Err(o) => match o.downcast::<(M, Option<Sender<R>>)>() {
                                    Ok(p) => {
                                        let (m, s) = *p;
                                        let r = handler(&mut state, m);
                                        if let Some(s) = s {
                                            let _ = s.send(r);
                                        }
                                    }
                                    _ => {
                                        // Here we intentionally drop the sender to
                                        // signal to the receiver that we failed to
                                        // handle this type of message.
                                    }
                                },
                            }
                        }
                        _ => return,
                    }
                }
            })
            .detach();
        appliance
    }
}

fn to_result<T>(r: Result<(), TrySendError<T>>) -> Result<(), Error> {
    match r {
        Err(TrySendError::Full(_)) => Err(Error::FullBuffer),
        Err(TrySendError::Disconnected(_)) => Err(Error::Stopped),
        _ => Ok(()),
    }
}
