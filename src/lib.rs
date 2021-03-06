//! A lightweight actor model inspired framework to build
//! customizable components with message-based intercommunications.

#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]
mod error;

use std::{
    any::type_name,
    fmt::{self, Debug},
    panic::catch_unwind,
    sync::{Arc, Weak},
    thread,
    time::Duration,
};
use {
    async_io::block_on,
    flume::{bounded, unbounded, Receiver, RecvTimeoutError, Sender, TrySendError},
    futures_lite::future::{or, pending},
    once_cell::sync::Lazy,
};

pub use crate::error::Error;

/// An async executor.
pub type Executor<'a> = async_executor::Executor<'a>;

/// A default executor. It runs on per-core threads and is fair
/// in terms of task priorities.
pub static DEFAULT_EXECUTOR: Lazy<Executor<'static>> = Lazy::new(|| {
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

/// `Message` must be implemented for any type which is intended for
/// sending to appliances.
///
/// # Example
/// ```
/// # use std::time::Duration;
/// # use appliance::{Appliance, ApplianceHandle, Handler, Message};
/// type Counter = Appliance<'static, usize>;
///
/// struct Ping;
///
/// impl Message for Ping { type Result = usize; }
///
/// impl Handler<Ping> for Counter {
///     fn handle(&mut self, _msg: &Ping) -> usize {
///         *self.state() += 1;
///         *self.state()
///     }
/// }
///
/// fn do_ping(handle: ApplianceHandle<Counter>) {
///     match handle.send_and_wait_with_timeout(Ping, Duration::from_secs(10)) {
///         Ok(cnt) => println!("Appliance was pinged successfully {} times", *cnt),
///         Err(err) => panic!("Ping to appliance has failed: {}", err),
///     }
/// }
/// ```
pub trait Message: Send {
    /// The type of replies generated by handling this message.
    type Result: Send;
}

/// A trait which must be implemented for all appliances which are intended to receive
/// messages of type `M`. One appliance can handle multiple message types.
/// 
/// Handler's logic is strongly encouraged to include only fast (non-blocking) and synchronous
/// mutations of the appliance state. Otherwise, the appliance's event loop may get slow, and
/// hence flood the internal buffer causing message sending denials.
///
/// # Example
/// ```
/// # use std::time::Duration;
/// # use appliance::{Appliance, ApplianceHandle, DEFAULT_EXECUTOR, Handler, Message};
/// type Counter = Appliance<'static, usize>;
///
/// struct Ping;
///
/// impl Message for Ping { type Result = usize; }
///
/// impl Handler<Ping> for Counter {
///     fn handle(&mut self, _msg: &Ping) -> usize {
///         *self.state() += 1;
///         *self.state()
///     }
/// }
///
/// struct Reset;
///
/// impl Message for Reset { type Result = (); }
///
/// impl Handler<Reset> for Counter {
///     fn handle(&mut self, _msg: &Reset) {
///         *self.state() = 0;
///     }
/// }
///
/// const BUF_SIZE: usize = 10;
///
/// fn main() -> Result<(), appliance::Error> {
///     let handle = Appliance::new_bounded(&DEFAULT_EXECUTOR, 0, BUF_SIZE);
///     assert_eq!(*handle.send_and_wait_sync(Ping)?, 1);
///     assert_eq!(*handle.send_and_wait_sync(Ping)?, 2);
///     handle.send(Reset)?;
///     assert_eq!(*handle.send_and_wait_sync(Ping)?, 1);
///     Ok(())
/// }
/// ```
pub trait Handler<M: Message> {
    /// Handle the incoming message
    fn handle(&mut self, msg: M) -> M::Result;
}

/// A dual trait for `Handler`. For any type of messages `M` and any type of handlers `H`,
/// if `impl Handle<M> for H`, then `impl HandledBy<H> for M`. I.e. we can either ask
/// "which messages can be handled by this appliance" or "which actors can handle this message",
/// and the answers to these questions are dual. The trait `HandledBy` answers the second
/// question.
///
/// Normally one should always implement `Handle<M>`, unless for some reason it is impossible
/// to do. The dual `HandledBy` impl is then provided automatically.
///
/// Generic methods, on the other hand, should use the trait constraint `T: HandledBy<H>`, since
/// the set of types for which `T: HandledBy<H>` is strictly larger than those for which
/// `H: Handler<T>`. An example where the client would need to implement `HandledBy` is if they
/// want to add custom messages for a library-provided handler type.
pub trait HandledBy<H: ?Sized>: Message {
    /// Handle the given message with the provided handler.
    ///
    /// The return type is wrapped in order to remove generic parameters from this function.
    /// The actual result value can be recovered with `ResultWrapper::downcast` if the type
    /// of the result is known.
    fn handle_by(self, handler: &mut H) -> Self::Result;
}

impl<H, M: Message> HandledBy<H> for M
where
    H: Handler<M>,
{
    fn handle_by(self, handler: &mut H) -> Self::Result {
        handler.handle(self)
    }
}

struct InnerMessage<'a, H: ?Sized + 'a> {
    handle_message: Box<dyn FnOnce(&mut H) + Send + 'a>,
}

impl<'a, H: ?Sized + 'a> InnerMessage<'a, H> {
    fn new<M>(message: M, reply_channel: Option<Sender<M::Result>>) -> Self
    where
        M: HandledBy<H> + 'a,
    {
        InnerMessage {
            handle_message: Box::new(move |handler| {
                let result = message.handle_by(handler);
                if let Some(rc) = &reply_channel {
                    rc.send(result).ok();
                }
            }),
        }
    }
}

type MessageSender<'a, H> = Sender<InnerMessage<'a, H>>;

/// A stateful entity that only allows to interact with via handling messages.
///
/// The appliance itself is not directly available. Messages must be sent to it
/// using its handle which is returned by the `Appliance::new_bounded` and
/// `Appliance::new_unbounded` methods, and can also be obtained from `&Appliance`
/// using `Appliance::handle` method. Note that the latter route is generally
/// available only for message handler `Handler` implementations.
pub struct Appliance<'s, S> {
    state: S,
    handle: Weak<MessageSender<'s, Self>>,
}

impl<'s, S: Debug + 's> Debug for Appliance<'s, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Appliance({:?})", self.state)
    }
}

impl<'s, S> Appliance<'s, S>
where
    S: Send + 's,
{
    /// Creates a new appliance with a bounded message buffer,
    /// a state, and a handler.
    pub fn new_bounded(
        executor: &'s Executor<'s>,
        state: S,
        size: usize,
    ) -> ApplianceHandle<'s, Self> {
        Self::run(executor, state, size)
    }

    /// Creates a new appliance with an unbounded message buffer,
    /// a state, and a handler. It's not recommended to use this
    /// version of appliance in production just like any other
    /// memory unbounded contruct.
    pub fn new_unbounded(executor: &'s Executor<'s>, state: S) -> ApplianceHandle<'s, Self> {
        Self::run(executor, state, None)
    }

    /// Creates a new appliance with the given state, message handler,
    /// and buffer size, if any.
    fn run(
        executor: &'s Executor<'s>,
        state: S,
        size: impl Into<Option<usize>>,
    ) -> ApplianceHandle<'s, Self> {
        let (in_, out_) = if let Some(mbs) = size.into() {
            bounded(mbs)
        } else {
            unbounded()
        };
        let handle = ApplianceHandle {
            inner: Arc::new(in_),
        };
        let mut appliance = Appliance {
            state,
            handle: Arc::downgrade(&handle.inner),
        };
        executor
            .spawn(async move { appliance.handle_messages(out_).await })
            .detach();
        handle
    }

    /// Returns a handle object of the appliance.
    ///
    /// A handle is a cloneable object which allows to send messages to the appliance.
    ///
    /// This function will return `None` if all appliance handles were already dropped.
    /// In this case the appliance must shutdown.
    pub fn handle(&'s self) -> Option<ApplianceHandle<'s, Self>> {
        self.handle.upgrade().map(|inner| ApplianceHandle { inner })
    }

    /// The mutable inner state of the appliance.
    ///
    /// Note that this function requires mutable access to the appliance itself (not its
    /// handle), but the appliance object is never returned by the API. The only place where
    /// the appliance can be accessed is the implementation of `Handle` and `HandledBy` traits
    /// for the message types, which is thus also the only place where one can (and should) mutate its state.
    pub fn state(&mut self) -> &mut S {
        &mut self.state
    }

    async fn handle_messages(&mut self, out_: Receiver<InnerMessage<'_, Self>>) {
        while let Ok(InnerMessage { handle_message }) = out_.recv_async().await {
            handle_message(self);
        }
    }
}

/// Appliance handle is a cloneable object which allows to send messages to the appliance.
///
/// Once all handles to the appliance are dropped, the appliance will terminate its event loop
/// and be destroyed.
pub struct ApplianceHandle<'a, A: ?Sized> {
    /// The incoming channel which is used to send messages to the appliance.
    ///
    /// We are forced to stupidly wrap `Sender` in an `Arc` even though it already is a
    /// wrapped `Arc`. We need the extra `Arc` so that we can pass a weak reference to it into
    /// the `Appliance` object, but unfortunately `flume::Sender` doesn't provide weak references
    /// in the API.
    ///
    /// Make sure that the inner `Sender` is never leaked outside of the containing `Arc`. If that
    /// happens, the appliance will stay alive after all handles are dropped, which violates the
    /// API contract.
    inner: Arc<MessageSender<'a, A>>,
}

impl<'a, A: ?Sized + 'a> Debug for ApplianceHandle<'a, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ApplianceHandle<{}>(..)", type_name::<A>())
    }
}

impl<'a, A: ?Sized + 'a> Clone for ApplianceHandle<'a, A> {
    fn clone(&self) -> Self {
        ApplianceHandle {
            inner: self.inner.clone(),
        }
    }
}

impl<'a, A: ?Sized + 'a> ApplianceHandle<'a, A> {
    /// Sends a message to the current appliance without
    /// waiting for the message to be handled.
    pub fn send_sync<M>(&self, message: M) -> Result<(), Error>
    where
        M: HandledBy<A> + 'a,
    {
        match self.inner.try_send(InnerMessage::new(message, None)) {
            Ok(_) => Ok(()),
            Err(TrySendError::Full(_)) => Err(Error::FullBuffer),
            Err(TrySendError::Disconnected(_)) => Err(Error::UnexpectedFailure),
        }
    }

    /// Does conceptually the same thing as `send_sync` but gets intended
    /// to be used in async context.
    pub async fn send_async<M>(&self, message: M) -> Result<(), Error>
    where
        M: HandledBy<A> + 'a,
    {
        self.inner
            .send_async(InnerMessage::new(message, None))
            .await
            .map_err(|_| Error::UnexpectedFailure)
    }

    /// Sends a message to the current appliance and waits
    /// forever, if `timeout` is None, or for only given time
    /// for the message to be handled.
    /// This synchronous blocking method is a fit for callers
    /// who don't use async execution and must be assured that
    /// the message has been handled.
    /// Note, it is supposed to be used less often than `send`
    /// as it may suffer a significant performance hit due to
    /// synchronization with the handling loop.
    pub fn send_and_wait_sync<M>(
        &self,
        message: M,
        timeout: Option<Duration>,
    ) -> Result<M::Result, Error>
    where
        M: HandledBy<A> + 'a,
    {
        let (s, r) = bounded(1);
        match self.inner.try_send(InnerMessage::new(message, Some(s))) {
            Err(TrySendError::Full(_)) => return Err(Error::FullBuffer),
            Err(TrySendError::Disconnected(_)) => return Err(Error::UnexpectedFailure),
            _ => {}
        }
        if let Some(timeout) = timeout {
            match r.recv_timeout(timeout) {
                Ok(r) => Ok(r),
                Err(RecvTimeoutError::Timeout) => Err(Error::Timeout),
                Err(RecvTimeoutError::Disconnected) => Err(Error::UnexpectedFailure),
            }
        } else {
            r.recv().map_err(|_| Error::UnexpectedFailure)
        }
    }

    /// Does conceptually the same thing as `send_and_wait_sync`
    /// but gets intended to be used in async context. This method
    /// is well suited for waiting for a result.
    pub async fn send_and_wait_async<M>(
        &self,
        message: M,
        timeout: Option<Duration>,
    ) -> Result<M::Result, Error>
    where
        M: HandledBy<A> + 'a,
    {
        let (send, recv) = bounded(1);
        if let Err(_) = self
            .inner
            .send_async(InnerMessage::new(message, Some(send)))
            .await
        {
            return Err(Error::UnexpectedFailure);
        }
        if let Some(timeout) = timeout {
            let f1 = async {
                recv.recv_async()
                    .await
                    .map_err(|_| Error::UnexpectedFailure)
            };
            let f2 = async {
                async_io::Timer::after(timeout).await;
                Err(Error::Timeout)
            };
            or(f1, f2).await
        } else {
            recv.recv_async()
                .await
                .map_err(|_| Error::UnexpectedFailure)
        }
    }
}
