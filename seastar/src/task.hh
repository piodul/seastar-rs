#pragma once

#include <cstddef>
#include <atomic>

// A function that polls a given Rust future behind the pointer that returns
// nothing, i.e. returns a unit.
//
// The task pointer is intended to be used to construct a waker from Rust.
//
// If the future becomes ready:
// - Destroys and deallocates the future,
// - Returns 1.
// Otherwise returns 0.
using rust_future_poll_fn = int (*)(void* task, void* future);

using rust_spawner_fn = void* (*)(void* data);

extern "C" {

// Spawns a task that takes given Rust future and polls it to completion
// using poll_fn.
void seastar_rs_spawn(rust_future_poll_fn poll_fn, void* rust_future);

// Spawns a task using given FnOnce on a given shard.
void seastar_rs_submit_to(
    rust_future_poll_fn poll_fn, rust_spawner_fn spawn_fn,
    void* data, unsigned shard);

void seastar_rs_waker_clone(void* data);
void seastar_rs_waker_wake(void* data);
void seastar_rs_waker_wake_by_ref(void* data);
void seastar_rs_waker_dispose(void* data);

}

