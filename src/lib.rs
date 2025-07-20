/*!
Provides a Swift-style `AsyncIterator.Continuation`-style channel for Rust.

This is the spiritual sibling to the [continue](crates.io/continue) crate, but for streams of data.

Like the continue crate:
* The send-side is synchronous, and the receive-side is asynchronous, as this crate is also designed
  as a primitive used to bridge synchronous and asynchronous code.
* Cancellation is thoughtfully supported and considered in the design.
* The channel is a single-producer, single-consumer pair.  So bring your own Clone, or consider
  more complex solutions if you need multiple producers or consumers.

Unlike the continue crate:
* The channel is buffered, allowing the sender to send multiple items before the receiver polls.
  Additional buffering policies are planned for the future.
* Dropping an unsent sender does not panic but instead terminates the stream by producing `None`
  on the receiver.

Unlike more complex channel types:
* Avoiding Clone simplifies the problem and unlocks the potential for more efficient
  implementations.
* Focus on bridiging sync and async code makes threading behavior more defined and predictable.

 */

use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;
use atomic_waker::AtomicWaker;

#[derive(Debug)]
struct SharedLock<T> {
    buffer: Vec<T>,
}
#[derive(Debug)]
struct Shared<T> {
    lock: Mutex<SharedLock<T>>,
    waker: AtomicWaker,
    sender_dropped: AtomicBool,
    receiver_dropped: AtomicBool,
}

#[derive(Debug)]
pub struct Sender<T> {
    shared: Arc<Shared<T>>,
}
#[derive(Debug)]
pub struct Receiver<T> {
    shared: Arc<Shared<T>>,
}

pub fn continuation<R>() -> (Sender<R>, Receiver<R>) {
    let shared = Arc::new(Shared {
        lock: Mutex::new(SharedLock { buffer: Vec::new() }),
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
    pub fn send(&self, item: T) {
        self.shared.lock.lock().unwrap().buffer.push(item);
        self.shared.waker.wake();
    }
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

impl<T> Receiver<T> {
    pub fn receive(&self) -> ReceiveFuture<T> {
        ReceiveFuture {
            shared: self.shared.clone(),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.shared.lock.lock().unwrap().buffer.is_empty()
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.shared.receiver_dropped.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

pub struct ReceiveFuture<T> {
    shared: Arc<Shared<T>>,
}

impl<T> std::future::Future for ReceiveFuture<T> {
    type Output = Option<T>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut lock = self.shared.lock.lock().unwrap();
        if let Some(item) = lock.buffer.pop() {
            std::task::Poll::Ready(Some(item))
        }
        else if self.shared.sender_dropped.load(std::sync::atomic::Ordering::Relaxed) {
            std::task::Poll::Ready(None)
        }
        else {
            self.shared.waker.register(cx.waker());
            std::task::Poll::Pending
        }
    }
}

impl<T> futures::Stream for Receiver<T> {
    type Item = T;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        let mut lock = self.shared.lock.lock().unwrap();
        if let Some(item) = lock.buffer.pop() {
            std::task::Poll::Ready(Some(item))
        } else if self.shared.sender_dropped.load(std::sync::atomic::Ordering::Relaxed) {
            std::task::Poll::Ready(None)
        } else {
            self.shared.waker.register(cx.waker());
            std::task::Poll::Pending
        }
    }
}

