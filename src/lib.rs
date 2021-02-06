//! A lightweight actor model inspired framework to build
//! customizable componets with message-based intercommunications.

#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]

use std::{error, fmt, panic::catch_unwind, thread};
use {
    async_io::block_on,
    event_listener::Event,
    flume::{bounded, unbounded, Sender, TrySendError},
    futures_lite::future::pending,
    once_cell::sync::Lazy,
};

/// An error describing appliance failures.
#[derive(Debug)]
pub enum Error {
    /// Indicates that a message was not sent because
    /// of the appliance's buffer being full.
    FullBuffer,
    /// Indicates that the appliance's handling loop is stopped.
    /// This is a fatal failure which signals that the appliance is
    /// irreparable and should not be further used.
    Stopped,
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::FullBuffer => write!(f, "appliance buffer is full"),
            Error::Stopped => write!(f, "appliance is stopped"),
        }
    }
}

/// An async executor.
pub type Executor<'a> = async_executor::Executor<'a>;

/// A default executor.
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

/// A stateful entity that only allows to
/// interact with via handling messages.
#[derive(Debug)]
pub struct Appliance<M> {
    in_: Sender<(M, Option<Event>)>,
}

impl<'a, M: Send + 'static> Appliance<M> {
    /// Creates a new appliance with a bounded message buffer,
    /// a state, and a handler.
    pub fn new_bounded<S: Send + 'a>(
        executor: &Executor<'a>,
        state: S,
        handler: impl Fn(&mut S, M) + Send + 'a,
        size: usize,
    ) -> Self {
        Self::new(executor, state, handler, Some(size))
    }

    /// Creates a new appliance with an unbounded message buffer,
    /// a state, and a handler.
    pub fn new_unbounded<S: Send + 'a>(
        executor: &Executor<'a>,
        state: S,
        handler: impl Fn(&mut S, M) + Send + 'a,
    ) -> Self {
        Self::new(executor, state, handler, None)
    }

    /// Sends a message to the current appliance without
    /// waiting for the message to be handled.
    pub fn send(&self, message: M) -> Result<(), Error> {
        to_result(self.in_.try_send((message, None)))
    }

    /// Sends a message to the current appliance and waits
    /// indefinitely for the message to be handled.
    /// This blocking method is a fit for callers who
    /// must be assured that the message has been handled.
    /// It is supposed to be used less often then `send` as
    /// it may suffer a performance hit due to synchronization
    /// with the handling loop.
    pub fn send_and_wait(&self, message: M) -> Result<(), Error> {
        let event = Event::new();
        let event_listener = event.listen();
        let result = to_result(self.in_.try_send((message, Some(event))));
        if result.is_ok() {
            event_listener.wait();
        }
        result
    }

    /// Creates a new appliance.
    fn new<S: Send + 'a>(
        executor: &Executor<'a>,
        mut state: S,
        handler: impl Fn(&mut S, M) + Send + 'a,
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
                        Ok((message, event)) => {
                            handler(&mut state, message);
                            if let Some(event) = event {
                                event.notify(1);
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

fn to_result<T>(res: Result<(), TrySendError<T>>) -> Result<(), Error> {
    match res {
        Err(TrySendError::Full(_)) => Err(Error::FullBuffer),
        Err(TrySendError::Disconnected(_)) => Err(Error::Stopped),
        _ => Ok(()),
    }
}
