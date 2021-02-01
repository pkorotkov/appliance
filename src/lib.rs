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

/// A stateful entity that only allows to
/// interact with via handling messages.
#[derive(Debug)]
pub struct Appliance<M> {
    messages_in: Sender<M>,
}

impl<'a, M: Send + 'static> Appliance<M> {
    /// Creates a new appliance with a bounded message buffer.
    pub fn new_bounded<S: Send + 'a>(
        executor: &Executor<'a>,
        state: S,
        handler: impl Fn(&mut S, M) + Send + 'a,
        size: usize,
    ) -> Self {
        Self::new(executor, state, handler, Some(size))
    }

    /// Creates a new appliance with an unbounded message buffer.
    pub fn new_unbounded<S: Send + 'a>(
        executor: &Executor<'a>,
        state: S,
        handler: impl Fn(&mut S, M) + Send + 'a,
    ) -> Self {
        Self::new(executor, state, handler, None)
    }

    /// Sends a message to the current appliance for processing.
    pub fn send(&self, message: M) -> bool {
        match self.messages_in.try_send(message) {
            Err(_) => false,
            _ => true,
        }
    }

    /// Creates a new appliance.
    fn new<S: Send + 'a>(
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
                        Ok(message) => handler(&mut state, message),
                        _ => return,
                    }
                }
            })
            .detach();
        appliance
    }
}
