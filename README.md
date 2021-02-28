# Appliance

[![Cargo](https://img.shields.io/crates/v/appliance.svg)](https://crates.io/crates/appliance)
[![Documentation](https://docs.rs/appliance/badge.svg)](https://docs.rs/appliance)
![minimum rustc 1.49](https://img.shields.io/badge/rustc-1.49+-red.svg)

## Overview

Appliance is a lightweight Rust framework for building highly customizable asynchronous components adapted for message-based intercommunications. This project is an attempt to make actix-like approach more flexible by exempting a user from using predefined execution runtimes. With the library you can design any composition logic for async agents avoiding data races and unnecessary locks.

__Features__

* Explicit control over agent lifecycle (no global runtime).
* Equipping agents with customized async executors.
* Minimal overhead when calling handlers.

## Installation

The recommended way to use this library is to add it as a dependency in your `Cargo.toml` file:

```
[dependencies]
appliance = "0.2.0"
```
