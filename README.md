# Appliance

[![Cargo](https://img.shields.io/crates/v/appliance.svg)](https://crates.io/crates/appliance)
![minimum rustc 1.49](https://img.shields.io/badge/rustc-1.49+-red.svg)

## Overview

Appliance is a lightweight Rust framework for building highly customizable asynchronous components adapted for message-based intercommunications. This project is an attempt to simplify actix-like approach exempting a user from using predefined execution runtimes. With the library you can design any composition logic for async agents avoiding data races and unnecessary locks.

## A quick ping-pong example

```rust
use appliance::Appliance;

type PingAppliance = Appliance<PingMessage>;
type PongAppliance = Appliance<PongMessage>;

enum PingMessage {
	RegisterPong(PongAppliance),
	Send(i16),
}

struct PingState {
	pong: Option<PongAppliance>,
}

fn ping_handler(state: &mut PingState, message: PingMessage) {
	match message {
		PingMessage::RegisterPong(pong) => state.pong = Some(pong),
		PingMessage::Send(c) => {
			if let Some(pong) = &state.pong {
				println!("ping received {}", c);
				let _ = pong.handle(PongMessage::Send(c+1));
				std::thread::sleep(std::time::Duration::from_secs(1));
			}
		}
	}
}

enum PongMessage {
	RegisterPing(PingAppliance),
	Send(i16),
}

struct PongState {
	ping: Option<PingAppliance>,
}

fn pong_handler(state: &mut PongState, message: PongMessage) {
	match message {
		PongMessage::RegisterPing(ping) => state.ping = Some(ping),
		PongMessage::Send(c) => {
			if let Some(ping) = &state.ping {
				println!("pong received {}", c);
				let _ = ping.handle(PingMessage::Send(c+1));
				std::thread::sleep(std::time::Duration::from_secs(1));
			}
		}
	}
}

fn main() {
	// Get the default (fair) executor.
	let executor = &appliance::DEFAULT_EXECUTOR;
	// Create a ping appliance.
	let ping_state = PingState { pong: None };
	let ping = Appliance::new(executor, ping_state, ping_handler, None);
	// Create a pong appliance.
	let pong_state = PongState { ping: None };
	let pong = Appliance::new(executor, pong_state, pong_handler, None);
	// Register appliances at one another.
	let _ = ping.handle(PingMessage::RegisterPong(pong.clone()));
	let _ = pong.handle(PongMessage::RegisterPing(ping.clone()));
	// Ignite the ping-pong interaction.
	let _ = ping.handle(PingMessage::Send(0));
	// Wait a bit to let the appliances talk.
	std::thread::sleep(std::time::Duration::from_secs(10));
}

```

## Installation

The recommended way to use this library is to add it as a dependency in your `Cargo.toml` file:

```
[dependencies]
appliance = "0.1.0"
```
