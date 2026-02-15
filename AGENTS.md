# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`continue_stream` is a Rust crate that provides a lightweight, single-producer single-consumer (SPSC) channel designed for bridging synchronous and asynchronous code. It's the streaming counterpart to the `continue` crate.

## Common Development Tasks

### Build and Check
```bash
cargo check            # Check if project compiles
cargo build           # Build the project
cargo build --release # Build optimized version
```

### Testing
```bash
cargo test            # Run all tests (unit tests and doctests)
cargo test --doc      # Run only doctests
cargo test --lib      # Run only library tests
```

### Code Quality
```bash
cargo fmt             # Format code
cargo clippy          # Run linter
cargo doc --no-deps   # Generate documentation
```

### Running a Single Test
```bash
cargo test test_name  # Run specific test by name
cargo test --test test_file_name  # Run specific test file
```

## Architecture and Key Concepts

### Core Components

The crate is structured around three main types:

1. **`continuation<T>()` function** - Creates a new channel pair
2. **`Sender<T>`** - Synchronous sending half, buffered and non-blocking
3. **`Receiver<T>`** - Asynchronous receiving half, implements `futures::Stream`

### Internal Structure

The channel uses a shared state pattern with:
- `SharedLock<T>` - Contains the `VecDeque<T>` buffer for queued items
- `Shared<T>` - Manages synchronization with:
  - `Mutex<SharedLock<T>>` for thread-safe buffer access
  - `AtomicWaker` for async task notification
  - Two `AtomicBool` flags tracking dropped status of sender/receiver

### Key Design Decisions

1. **no_std support**: The crate is `no_std` compatible, using only `alloc`
2. **Single-producer single-consumer**: No `Clone` implementation to keep it simple and efficient
3. **Buffered channel**: Multiple items can be sent before receiver polls
4. **Graceful termination**: Dropping sender signals stream completion (returns `None`)
5. **Cancellation detection**: Both sides can check if the other has been dropped

### Dependencies

- `atomic-waker` - For waking async tasks
- `futures` - For Stream trait implementation
- `wasm_safe_thread` - Cross-platform mutex (`Mutex::lock_sync()`) and thread primitives that work in WASM
- `test_executors` (dev) - For async test execution
- `wasm-bindgen-test` (dev, wasm32 only) - For browser-based WASM testing

## Testing Approach

The crate uses `test_executors` for async testing. Tests are marked with `#[test_executors::async_test]` which automatically runs them on multiple executors for compatibility testing.

For wasm32 tests, `wasm_safe_thread` is used in place of `std::thread` via conditional imports, and tests are configured to run in-browser with `wasm_bindgen_test_configure!(run_in_browser)`.