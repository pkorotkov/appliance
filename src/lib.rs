//! A lightweight actor model inspired framework to build
//! customizable componets with message-based intercommunications.

#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]

#[macro_use]
extern crate smallbox;

use async_io::block_on;
use flume::{bounded, unbounded, Sender};
use futures_lite::future::pending;
use once_cell::sync::Lazy;
use smallbox::SmallBox;
use std::{any::Any, panic::catch_unwind, thread};

/// A size of the capacity reserved for storing objects on stack.
/// It's currently 32 * sizeof(usize) = 32 * 8 = 256 bytes.
type Space = smallbox::space::S32;

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
pub struct Appliance {
    messages_in: Sender<SmallBox<dyn Any + Send, Space>>,
}

impl<'a> Appliance {
    /// Creates a new appliance with a bounded message buffer.
    pub fn new_bounded<S: Send + 'a, M: Send + 'static>(
        executor: &Executor<'a>,
        state: S,
        handler: impl Fn(&mut S, M) + Send + 'a,
        message_bus_size: usize,
    ) -> Self {
        Self::new(executor, state, handler, Some(message_bus_size))
    }

    /// Creates a new appliance with an unbounded message buffer.
    pub fn new_unbounded<S: Send + 'a, M: Send + 'static>(
        executor: &Executor<'a>,
        state: S,
        handler: impl Fn(&mut S, M) + Send + 'a,
    ) -> Self {
        Self::new(executor, state, handler, None)
    }

    /// Processes a message on the current appliance.
    pub fn handle<M: Send + 'static>(&self, message: M) -> Result<(), Error> {
        let m: SmallBox<dyn Any + Send, Space> = smallbox!(message);
        match self.messages_in.try_send(m) {
            Err(_) => Err(Error::HandlingFailure),
            _ => Ok(()),
        }
    }

    /// Creates a new appliance.
    fn new<S: Send + 'a, M: Send + 'static>(
        executor: &Executor<'a>,
        mut state: S,
        handler: impl Fn(&mut S, M) + Send + 'a,
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
                        Ok(message) => {
                            if let Ok(m) = message.downcast::<M>() {
                                handler(&mut state, m.into_inner());
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
