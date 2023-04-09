#[cxx::bridge(namespace = "seastar")]
mod ffi {
    unsafe extern "C++" {
        include!("seastar/core/reactor.hh");

        /// Checks whether the current thread is within a Seastar runtime.
        fn engine_is_ready() -> bool;
    }
}

pub use ffi::engine_is_ready;

/// Intended to be used in a runtime-dependent function.
/// Panics if called outside of a Seastar runtime.
pub fn assert_runtime_is_running() {
    if !engine_is_ready() {
        panic!("Attempting to call a runtime-dependent function outside of a Seastar runtime");
    }
}

/// Mainly intended to be used in [`seastar::AppTemplate::run_void`] and  [`seastar::AppTemplate::run_int`].
/// Panics if called within a Seastar runtime.
pub fn assert_runtime_is_not_running() {
    if engine_is_ready() {
        panic!("Attempting to call a function inside of a Seastar runtime that must be used outside of it");
    }
}
