# Appliance

[![Cargo](https://img.shields.io/crates/v/appliance.svg)](https://crates.io/crates/appliance)
[![Documentation](https://docs.rs/appliance/badge.svg)](https://docs.rs/appliance)
![minimum rustc 1.49](https://img.shields.io/badge/rustc-1.49+-red.svg)

## Overview

Appliance is a lightweight Rust framework for building highly customizable asynchronous components adapted for message-based intercommunications. This project is an attempt to make actix-like approach more flexible by exempting a user from using predefined execution runtimes. With the library you can design any composition logic for async agents avoiding data races and unnecessary locks.

__Features__

* Explicit control over agent lifecycle (no global runtime).
* Equipping agents with customized async executors.
* Minimal overhead when calling handlers (no traits used).

## Installation

The recommended way to use this library is to add it as a dependency in your `Cargo.toml` file:

```
[dependencies]
appliance = "0.1.7"
```

## A quick ping-pong example

```rust
use appliance::Appliance;
use std::{
    sync::{Arc, Weak},
    thread,
    time::Duration,
};

enum PingMessage {
    RegisterPong(Weak<Appliance>),
    Send(i16),
}

#[derive(Default)]
struct PingState {
    pong: Weak<Appliance>,
}

fn ping_handler(state: &mut PingState, message: PingMessage) {
    match message {
        PingMessage::RegisterPong(pong) => state.pong = pong,
        PingMessage::Send(c) => {
            if let Some(pong) = &state.pong.upgrade() {
                println!("ping received {}", c);
                let _ = pong.send(PongMessage::Send(c + 1));
                thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

enum PongMessage {
    RegisterPing(Weak<Appliance>),
    Send(i16),
}

#[derive(Default)]
struct PongState {
    ping: Weak<Appliance>,
}

fn pong_handler(state: &mut PongState, message: PongMessage) {
    match message {
        PongMessage::RegisterPing(ping) => state.ping = ping,
        PongMessage::Send(c) => {
            if let Some(ping) = &state.ping.upgrade() {
                println!("pong received {}", c);
                let _ = ping.send(PingMessage::Send(c + 1));
                thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

fn main() {
    let executor = &appliance::DEFAULT_EXECUTOR;
    // Create ping appliance.
    let ping_state: PingState = Default::default();
    let ping = Arc::new(Appliance::new_unbounded(executor, ping_state, ping_handler));
    // Create pong appliance.
    let pong_state: PongState = Default::default();
    let pong = Arc::new(Appliance::new_unbounded(executor, pong_state, pong_handler));
    // Cross-register appliances.
    let _ = ping.send(PingMessage::RegisterPong(Arc::downgrade(&pong)));
    let _ = pong.send(PongMessage::RegisterPing(Arc::downgrade(&ping)));
    // Ignite ping-pong.
    let _ = ping.send(PingMessage::Send(0));
    thread::sleep(Duration::from_secs(5));
}
```
