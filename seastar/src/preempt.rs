#[cxx::bridge(namespace = "seastar")]
mod ffi {
    unsafe extern "C++" {
        include!("seastar/core/preempt.hh");

        /// Checks whether the current task exhausted its time quota and should
        /// yield to the runtime as soon as possible.
        fn need_preempt() -> bool;
    }
}

use std::pin::Pin;
use std::task::{Context, Poll};

pub use ffi::need_preempt;
use futures::Future;

#[test]
fn test_preempt_smoke_test() {
    // The need_preempt function "works" even if there is no Seastar runtime
    // present. This test was only used to make sure that linking with Seastar
    // works properly. It can be removed later after we get some proper tests.
    assert!(!need_preempt());
    assert!(!need_preempt());
    assert!(!need_preempt());
}

#[inline]
pub fn yield_now() -> YieldFuture {
    YieldFuture { need_yield: true }
}

#[inline]
pub fn maybe_yield() -> YieldFuture {
    YieldFuture {
        need_yield: need_preempt(),
    }
}

#[derive(Debug)]
pub struct YieldFuture {
    need_yield: bool,
}

impl Future for YieldFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut s = self.as_mut();
        if s.need_yield {
            s.need_yield = false;
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

pub fn gentle_cleanup<F, T>(mut t: Option<T>, mut f: F)
where
    F: FnMut(T) -> Option<T> + 'static,
    T: 'static,
{
    // Do work now without preemption, until the end of the task quota
    while !need_preempt() {
        t = match t {
            Some(t) => f(t),
            None => return,
        }
    }

    // Don't spawn a task if there is nothing left to do
    if t.is_none() {
        return;
    }

    // Ran out of task quota, move the cleanup to a task
    let _ = crate::spawn(async move {
        loop {
            t = match t {
                Some(t) => f(t),
                None => return,
            };
            maybe_yield().await
        }
    });
}
