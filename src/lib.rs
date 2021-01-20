//! A lightweight actor model inspired framework to build
//! customizable componets with message-based intercommunications.

#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]

use async_io::block_on;
use flume::{bounded, unbounded, Sender};
use futures_lite::future::pending;
use once_cell::sync::Lazy;
use std::{panic::catch_unwind, thread};

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

/// An opaqueing error.
#[derive(Debug)]
pub enum Error {
    /// Exposes message handling errors.
    HandlingFailure,
}

/// A stateful entity that only allows to
/// interact with via handling messages.
#[derive(Debug)]
pub struct Appliance<M> {
    messages_in: Sender<M>,
}

impl<M> Clone for Appliance<M> {
    fn clone(&self) -> Self {
        Appliance {
            messages_in: self.messages_in.clone(),
        }
    }
}

impl<M: Send + 'static> Appliance<M> {
    /// Creates a new appliance.
    pub fn new<S: Send + 'static>(
        executor: &Executor<'_>,
        mut state: S,
        handler: impl Fn(&mut S, M) + Send + 'static,
        message_bus_size: Option<usize>,
    ) -> Self {
        let (messages_in, messages_out) = if let Some(mbs) = message_bus_size {
            bounded(mbs)
        } else {
            unbounded()
        };
        let appliance = Appliance { messages_in };
        executor
            .spawn(async move {
                loop {
                    match messages_out.recv_async().await {
                        Ok(message) => handler(&mut state, message),
                        Err(_) => return,
                    }
                }
            })
            .detach();
        appliance
    }

    /// Processes a message on the current appliance.
    pub fn handle(&self, message: M) -> Result<(), Error> {
        match self.messages_in.try_send(message) {
            Err(_) => Err(Error::HandlingFailure),
            Ok(_) => Ok(()),
        }
    }
}
