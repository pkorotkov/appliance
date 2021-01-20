# Appliance

[![Cargo](https://img.shields.io/crates/v/appliance.svg)](https://crates.io/crates/appliance)
![minimum rustc 1.49](https://img.shields.io/badge/rustc-1.49+-red.svg)

## Overview

Appliance is a lightweight Rust framework for building highly customizable asynchronous components adapted for message-based intercommunications. This project is an attempt to simplify actix-like approach exempting a user from using predefined execution runtimes. With the library you can design any composition logic for async agents avoiding data races and unnecessary locks.

## A quick ping-pong example

```rust
use appliance::Appliance;
use std::{
    sync::{Arc, Weak},
    thread,
    time::Duration,
};

type PingAppliance = Appliance<PingMessage>;
type PongAppliance = Appliance<PongMessage>;

enum PingMessage {
    RegisterPong(Weak<PongAppliance>),
    Send(i16),
}

struct PingState {
    pong: Weak<PongAppliance>,
}

impl Drop for PingState {
    fn drop(&mut self) {
        println!("dropping PingState");
    }
}

fn ping_handler(state: &mut PingState, message: PingMessage) {
    match message {
        PingMessage::RegisterPong(pong) => state.pong = pong,
        PingMessage::Send(c) => {
            if let Some(pong) = &state.pong.upgrade() {
                println!("ping received {}", c);
                let _ = pong.handle(PongMessage::Send(c + 1));
                thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

enum PongMessage {
    RegisterPing(Weak<PingAppliance>),
    Send(i16),
}

struct PongState {
    ping: Weak<PingAppliance>,
}

impl Drop for PongState {
    fn drop(&mut self) {
        println!("dropping PongState");
    }
}

fn pong_handler(state: &mut PongState, message: PongMessage) {
    match message {
        PongMessage::RegisterPing(ping) => state.ping = ping,
        PongMessage::Send(c) => {
            if let Some(ping) = &state.ping.upgrade() {
                println!("pong received {}", c);
                let _ = ping.handle(PingMessage::Send(c + 1));
                thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

fn main() {
    let executor = &appliance::DEFAULT_EXECUTOR;
    // Create ping appliance.
    let ping_state = PingState {
        pong: Default::default(),
    };
    let ping = Arc::new(Appliance::new(executor, ping_state, ping_handler, None));
    // Create pong appliance.
    let pong_state = PongState {
        ping: Default::default(),
    };
    let pong = Arc::new(Appliance::new(executor, pong_state, pong_handler, None));
    let _ = ping.handle(PingMessage::RegisterPong(Arc::downgrade(&pong)));
    let _ = pong.handle(PongMessage::RegisterPing(Arc::downgrade(&ping)));
    // Ignite ping-pong.
    let _ = ping.handle(PingMessage::Send(0));
    thread::sleep(Duration::from_secs(5));
    drop(ping);
    drop(pong);
    thread::sleep(Duration::from_secs(1));
}
```

## Installation

The recommended way to use this library is to add it as a dependency in your `Cargo.toml` file:

```
[dependencies]
appliance = "0.1.1"
```
