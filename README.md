# continue_stream

![logo](art/logo.png)

A Swift-style `AsyncIterator.Continuation`-style channel for Rust.

This crate provides a lightweight, single-producer single-consumer (SPSC) channel designed specifically for bridging synchronous and asynchronous code. It's the streaming counterpart to the [continue](https://crates.io/crates/continue) crate, optimized for sending multiple values over time rather than a single result.

## Key Features

* **Sync-to-async bridge**: Synchronous senders, asynchronous receivers
* **Buffered**: Multiple items can be sent before the receiver polls
* **Cancellation-aware**: Both sides can detect when the other has been dropped
* **Stream implementation**: Receivers implement `futures::Stream` for ergonomic async iteration
* **Lightweight**: No cloning overhead, optimized for single-producer single-consumer use
* **`no_std` compatible**: Works in embedded and WASM environments with `alloc`

## Design Philosophy

Like the continue crate:
* The send-side is synchronous, and the receive-side is asynchronous, as this crate is designed as a primitive used to bridge synchronous and asynchronous code.
* Cancellation is thoughtfully supported and considered in the design.
* The channel is a single-producer, single-consumer pair. Bring your own `Clone`, or consider more complex solutions if you need multiple producers or consumers.

Unlike the continue crate:
* The channel is buffered, allowing the sender to send multiple items before the receiver polls.
* Dropping an unsent sender does not panic but instead gracefully terminates the stream by producing `None` on the receiver.

Unlike more complex channel types:
* Avoiding `Clone` simplifies the problem and unlocks the potential for more efficient implementations.
* Focus on bridging sync and async code makes threading behavior more defined and predictable.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
continue_stream = "0.1.0"
```

For `no_std` environments (embedded, WASM):

```toml
[dependencies]
continue_stream = { version = "0.1.0", default-features = false }
```

### Platform Support

* **`no_std` compatible**: Works without the standard library, only requires `alloc`
* **Full functionality**: All features including threading work in both `std` and `no_std` environments

## Examples

### Basic Usage

```rust
use continue_stream::continuation;

let (sender, receiver) = continuation::<i32>();

// Send from synchronous code
sender.send(1);
sender.send(2);
sender.send(3);
drop(sender); // Signal completion

// Receive in async code
while let Some(value) = receiver.receive().await {
    println!("Received: {}", value);
}
```

### Using as a Stream

```rust
use continue_stream::continuation;
use futures::StreamExt;

let (sender, receiver) = continuation::<String>();

// Send some messages
sender.send("Hello".to_string());
sender.send("World".to_string());
drop(sender);

// Use Stream combinators
let messages: Vec<String> = receiver.collect().await;
assert_eq!(messages, vec!["Hello", "World"]);
```

### Cross-thread Communication

```rust
use continue_stream::continuation;
use std::thread;
use std::time::Duration;

let (sender, receiver) = continuation::<i32>();

// Spawn a thread that sends values
thread::spawn(move || {
    for i in 0..5 {
        sender.send(i);
        thread::sleep(Duration::from_millis(10));
    }
    // Sender is dropped here, signaling completion
});

// Receive values asynchronously
let mut count = 0;
while let Some(value) = receiver.receive().await {
    assert_eq!(value, count);
    count += 1;
}
assert_eq!(count, 5);
```

### Cancellation Detection

```rust
use continue_stream::continuation;

let (sender, receiver) = continuation::<i32>();

// Drop the receiver to cancel
drop(receiver);

// Sender can detect cancellation
assert!(sender.is_cancelled());
```

## API

The crate provides three main types:

### `continuation<T>() -> (Sender<T>, Receiver<T>)`

Creates a new continuation channel, returning a sender-receiver pair.

### `Sender<T>`

The sending half of a continuation channel. Allows synchronous code to send values to an asynchronous receiver.

- `send(item: T)` - Sends a value through the channel (non-blocking)
- `is_cancelled() -> bool` - Checks if the receiver has been dropped

### `Receiver<T>`

The receiving half of a continuation channel. Provides asynchronous access to values sent through a `Sender`.

- `receive() -> ReceiveFuture<T>` - Receives a single value from the channel
- `is_cancelled() -> bool` - Checks if the sender has been dropped

Implements `futures::Stream` for ergonomic async iteration.

## License

This project is licensed under the MIT or Apache-2.0 license, at your option.
