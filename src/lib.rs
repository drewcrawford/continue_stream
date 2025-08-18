//! A Swift-style `AsyncIterator.Continuation`-style channel for Rust.
//!
//! This crate provides a lightweight, single-producer single-consumer (SPSC) channel designed
//! specifically for bridging synchronous and asynchronous code. It's the streaming counterpart
//! to the [continue](https://crates.io/crates/continue) crate, optimized for sending multiple
//! values over time rather than a single result.
//!
//! # Key Features
//!
//! * **Sync-to-async bridge**: Synchronous senders, asynchronous receivers
//! * **Buffered**: Multiple items can be sent before the receiver polls
//! * **Cancellation-aware**: Both sides can detect when the other has been dropped
//! * **Stream implementation**: Receivers implement `futures::Stream` for ergonomic async iteration
//! * **Lightweight**: No cloning overhead, optimized for single-producer single-consumer use
//!
//! # Design Philosophy
//!
//! Like the continue crate:
//! * The send-side is synchronous, and the receive-side is asynchronous, as this crate is designed
//!   as a primitive used to bridge synchronous and asynchronous code.
//! * Cancellation is thoughtfully supported and considered in the design.
//! * The channel is a single-producer, single-consumer pair. Bring your own `Clone`, or consider
//!   more complex solutions if you need multiple producers or consumers.
//!
//! Unlike the continue crate:
//! * The channel is buffered, allowing the sender to send multiple items before the receiver polls.
//! * Dropping an unsent sender does not panic but instead gracefully terminates the stream by
//!   producing `None` on the receiver.
//!
//! Unlike more complex channel types:
//! * Avoiding `Clone` simplifies the problem and unlocks the potential for more efficient
//!   implementations.
//! * Focus on bridging sync and async code makes threading behavior more defined and predictable.
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```
//! # async fn example() {
//! use continue_stream::continuation;
//!
//! let (sender, receiver) = continuation::<i32>();
//!
//! // Send from synchronous code
//! sender.send(1);
//! sender.send(2);
//! sender.send(3);
//! drop(sender); // Signal completion
//!
//! // Receive in async code
//! while let Some(value) = receiver.receive().await {
//!     println!("Received: {}", value);
//! }
//! # }
//! ```
//!
//! ## Using as a Stream
//!
//! ```
//! # async fn example() {
//! use continue_stream::continuation;
//! use futures::StreamExt;
//!
//! let (sender, receiver) = continuation::<String>();
//!
//! // Send some messages
//! sender.send("Hello".to_string());
//! sender.send("World".to_string());
//! drop(sender);
//!
//! // Use Stream combinators
//! let messages: Vec<String> = receiver.collect().await;
//! assert_eq!(messages, vec!["Hello", "World"]);
//! # }
//! ```
//!
//! ## Cross-thread Communication
//!
//! ```
//! # async fn example() {
//! use continue_stream::continuation;
//! use std::thread;
//! use std::time::Duration;
//!
//! let (sender, receiver) = continuation::<i32>();
//!
//! // Spawn a thread that sends values
//! thread::spawn(move || {
//!     for i in 0..5 {
//!         sender.send(i);
//!         thread::sleep(Duration::from_millis(10));
//!     }
//!     // Sender is dropped here, signaling completion
//! });
//!
//! // Receive values asynchronously
//! let mut count = 0;
//! while let Some(value) = receiver.receive().await {
//!     assert_eq!(value, count);
//!     count += 1;
//! }
//! assert_eq!(count, 5);
//! # }
//! ```
//!
//! ## Cancellation Detection
//!
//! ```
//! # async fn example() {
//! use continue_stream::continuation;
//!
//! let (sender, receiver) = continuation::<i32>();
//!
//! // Drop the receiver to cancel
//! drop(receiver);
//!
//! // Sender can detect cancellation
//! assert!(sender.is_cancelled());
//! # }
//! ```

use std::collections::VecDeque;
use std::sync::{Arc};
use wasm_safe_mutex::Mutex;
use std::sync::atomic::AtomicBool;
use atomic_waker::AtomicWaker;

#[derive(Debug)]
struct SharedLock<T> {
    buffer: VecDeque<T>,
}
#[derive(Debug)]
struct Shared<T> {
    lock: Mutex<SharedLock<T>>,
    waker: AtomicWaker,
    sender_dropped: AtomicBool,
    receiver_dropped: AtomicBool,
}

/// The sending half of a continuation channel.
///
/// `Sender` allows synchronous code to send values to an asynchronous receiver.
/// Values are buffered until the receiver polls for them. When the sender is
/// dropped, it signals completion to the receiver.
///
/// # Examples
///
/// ```
/// # async fn example() {
/// use continue_stream::continuation;
///
/// let (sender, receiver) = continuation::<String>();
///
/// // Send multiple values
/// sender.send("first".to_string());
/// sender.send("second".to_string());
///
/// // Check if receiver is still interested
/// if !sender.is_cancelled() {
///     sender.send("third".to_string());
/// }
///
/// drop(sender); // Signal completion
/// # }
/// ```
#[derive(Debug)]
pub struct Sender<T> {
    shared: Arc<Shared<T>>,
}
/// The receiving half of a continuation channel.
///
/// `Receiver` provides asynchronous access to values sent through a `Sender`.
/// It implements `futures::Stream` for ergonomic async iteration and provides
/// a `receive()` method for one-shot reception.
///
/// # Examples
///
/// ```
/// # async fn example() {
/// use continue_stream::continuation;
/// use futures::StreamExt;
///
/// let (sender, mut receiver) = continuation::<i32>();
///
/// sender.send(1);
/// sender.send(2);
/// drop(sender);
///
/// // Use as a Stream
/// while let Some(value) = receiver.next().await {
///     println!("Got: {}", value);
/// }
/// # }
/// ```
///
/// ```
/// # async fn example() {
/// use continue_stream::continuation;
///
/// let (sender, receiver) = continuation::<i32>();
///
/// sender.send(42);
/// drop(sender);
///
/// // Use receive() for one-shot reception
/// if let Some(value) = receiver.receive().await {
///     assert_eq!(value, 42);
/// }
/// # }
/// ```
#[derive(Debug)]
pub struct Receiver<T> {
    shared: Arc<Shared<T>>,
}

/// Creates a new continuation channel, returning a sender-receiver pair.
///
/// This function creates a buffered, single-producer single-consumer channel
/// designed for bridging synchronous and asynchronous code. The sender can
/// synchronously push values that the receiver asynchronously consumes.
///
/// # Type Parameters
///
/// * `R` - The type of values to be sent through the channel
///
/// # Returns
///
/// A tuple containing:
/// * `Sender<R>` - The sending half for synchronous sends
/// * `Receiver<R>` - The receiving half for asynchronous receives
///
/// # Examples
///
/// ```
/// # async fn example() {
/// use continue_stream::continuation;
///
/// // Create a channel for sending strings
/// let (sender, receiver) = continuation::<String>();
///
/// // Send a value
/// sender.send("Hello".to_string());
/// drop(sender);
///
/// // Receive the value
/// assert_eq!(receiver.receive().await, Some("Hello".to_string()));
/// # }
/// ```
///
/// ```
/// # async fn example() {
/// use continue_stream::continuation;
/// use std::thread;
///
/// // Create a channel for cross-thread communication
/// let (sender, receiver) = continuation::<i32>();
///
/// // Send from a thread
/// thread::spawn(move || {
///     for i in 0..3 {
///         sender.send(i);
///     }
/// });
///
/// // Receive asynchronously
/// let mut values = Vec::new();
/// while let Some(value) = receiver.receive().await {
///     values.push(value);
/// }
/// assert_eq!(values, vec![0, 1, 2]);
/// # }
/// ```
pub fn continuation<R>() -> (Sender<R>, Receiver<R>) {
    let shared = Arc::new(Shared {
        lock: Mutex::new(SharedLock { buffer: VecDeque::new() }),
        waker: AtomicWaker::new(),
        sender_dropped: AtomicBool::new(false),
        receiver_dropped: AtomicBool::new(false),
    });

    (
        Sender {
            shared: shared.clone(),
        },
        Receiver { shared },
    )
}

impl<T> Sender<T> {
    /// Sends a value through the channel.
    ///
    /// This method is synchronous and will not block. The value is buffered
    /// and will be delivered when the receiver polls for it. If the receiver
    /// has been dropped, the value is still accepted but will never be received.
    ///
    /// # Arguments
    ///
    /// * `item` - The value to send through the channel
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn example() {
    /// use continue_stream::continuation;
    ///
    /// let (sender, receiver) = continuation::<i32>();
    ///
    /// // Send multiple values
    /// sender.send(1);
    /// sender.send(2);
    /// sender.send(3);
    ///
    /// drop(sender); // Signal completion
    ///
    /// // Values are buffered and available for reception
    /// assert_eq!(receiver.receive().await, Some(1));
    /// assert_eq!(receiver.receive().await, Some(2));
    /// assert_eq!(receiver.receive().await, Some(3));
    /// assert_eq!(receiver.receive().await, None); // Stream ended
    /// # }
    /// ```
    pub fn send(&self, item: T) {
        self.shared.lock.lock_sync().buffer.push_back(item);
        self.shared.waker.wake();
    }
    
    /// Checks if the receiver has been dropped (cancelled).
    ///
    /// This method allows the sender to detect when the receiver is no longer
    /// interested in receiving values, enabling early termination of send operations.
    ///
    /// # Returns
    ///
    /// `true` if the receiver has been dropped, `false` otherwise
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn example() {
    /// use continue_stream::continuation;
    ///
    /// let (sender, receiver) = continuation::<i32>();
    ///
    /// assert!(!sender.is_cancelled());
    ///
    /// drop(receiver); // Cancel by dropping receiver
    ///
    /// assert!(sender.is_cancelled());
    ///
    /// // Could use this to stop sending
    /// if !sender.is_cancelled() {
    ///     sender.send(42); // This won't execute
    /// }
    /// # }
    /// ```
    pub fn is_cancelled(&self) -> bool {
        self.shared.receiver_dropped.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.shared.sender_dropped.store(true, std::sync::atomic::Ordering::Relaxed);
        self.shared.waker.wake();
    }
}

// ============================================================================
// Boilerplate trait implementations
// ============================================================================



impl<T> std::fmt::Display for Sender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sender {{ cancelled: {} }}", self.is_cancelled())
    }
}

impl<T> Receiver<T> {
    /// Receives a single value from the channel.
    ///
    /// Returns a future that resolves to the next value in the channel, or `None`
    /// if the sender has been dropped and all buffered values have been consumed.
    ///
    /// This method is useful for one-shot reception or when you need more control
    /// over the reception process than the `Stream` implementation provides.
    ///
    /// # Returns
    ///
    /// A `ReceiveFuture` that resolves to:
    /// * `Some(T)` - The next value from the channel
    /// * `None` - The channel is closed (sender dropped and buffer empty)
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn example() {
    /// use continue_stream::continuation;
    ///
    /// let (sender, receiver) = continuation::<String>();
    ///
    /// sender.send("Hello".to_string());
    /// sender.send("World".to_string());
    /// drop(sender);
    ///
    /// // Receive values one by one
    /// assert_eq!(receiver.receive().await, Some("Hello".to_string()));
    /// assert_eq!(receiver.receive().await, Some("World".to_string()));
    /// assert_eq!(receiver.receive().await, None); // Channel closed
    /// # }
    /// ```
    ///
    /// ```
    /// # async fn example() {
    /// use continue_stream::continuation;
    ///
    /// let (sender, receiver) = continuation::<i32>();
    ///
    /// // Send from another async task
    /// # let handle = async move {
    /// #     sender.send(1);
    /// #     sender.send(2); 
    /// #     sender.send(3);
    /// # };
    /// # handle.await;
    ///
    /// // Receive in a loop
    /// let mut sum = 0;
    /// while let Some(value) = receiver.receive().await {
    ///     sum += value;
    /// }
    /// assert_eq!(sum, 6);
    /// # }
    /// ```
    pub fn receive(&self) -> ReceiveFuture<T> {
        ReceiveFuture {
            shared: self.shared.clone(),
        }
    }

    /// Checks if the sender has been dropped (channel closed).
    ///
    /// This method allows the receiver to detect when the sender has been dropped,
    /// indicating that no more values will be sent. Note that there may still be
    /// buffered values to receive even if this returns `true`.
    ///
    /// # Returns
    ///
    /// `true` if the sender has been dropped, `false` otherwise
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn example() {
    /// use continue_stream::continuation;
    ///
    /// let (sender, receiver) = continuation::<i32>();
    ///
    /// sender.send(42);
    /// assert!(!receiver.is_cancelled());
    ///
    /// drop(sender);
    /// assert!(receiver.is_cancelled());
    ///
    /// // But we can still receive buffered values
    /// assert_eq!(receiver.receive().await, Some(42));
    /// # }
    /// ```
    pub fn is_cancelled(&self) -> bool {
        self.shared.sender_dropped.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.shared.receiver_dropped.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl<T> std::fmt::Display for Receiver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Receiver {{ cancelled: {} }}", self.is_cancelled())
    }
}

/// A future representing a pending receive operation.
///
/// This future is created by [`Receiver::receive()`] and resolves to the next
/// value from the channel, or `None` if the channel is closed.
///
/// # Examples
///
/// ```
/// # async fn example() {
/// use continue_stream::continuation;
///
/// let (sender, receiver) = continuation::<i32>();
///
/// // Create a receive future
/// let fut = receiver.receive();
///
/// // Send a value
/// sender.send(42);
///
/// // The future will resolve to the sent value
/// assert_eq!(fut.await, Some(42));
/// # }
/// ```
pub struct ReceiveFuture<T> {
    shared: Arc<Shared<T>>,
}

impl<T> std::future::Future for ReceiveFuture<T> {
    type Output = Option<T>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        //need to hold this open until registering the waker.
        let mut lock = self.shared.lock.lock_sync();
        if let Some(item) = lock.buffer.pop_front() {
            std::task::Poll::Ready(Some(item))
        }
        else if self.shared.sender_dropped.load(std::sync::atomic::Ordering::Relaxed) {
            std::task::Poll::Ready(None)
        }
        else {
            self.shared.waker.register(cx.waker());
            drop(lock);
            std::task::Poll::Pending
        }
    }
}

impl<T> futures::Stream for Receiver<T> {
    type Item = T;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        let mut lock = self.shared.lock.lock_sync();
        if let Some(item) = lock.buffer.pop_front() {
            std::task::Poll::Ready(Some(item))
        } else if self.shared.sender_dropped.load(std::sync::atomic::Ordering::Relaxed) {
            std::task::Poll::Ready(None)
        } else {
            self.shared.waker.register(cx.waker());
            std::task::Poll::Pending
        }
    }
}

#[cfg(test)] mod tests {
    use std::thread;
    use std::time::Duration;

    #[test_executors::async_test] async fn initially_empty() {
        let (sender, receiver) = super::continuation::<()>();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            sender.send(())
        });
        receiver.receive().await;
    }
}

