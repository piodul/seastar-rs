use std::mem::ManuallyDrop;

use futures::Future;

/// A container for an object that must be destroyed on the original shard.
pub struct Foreign<T: 'static> {
    inner: ManuallyDrop<T>,
    origin_shard: u32,
}

// Foreign is Send because it destroys the
unsafe impl<T: 'static> Send for Foreign<T> {}

struct AssertSend<T>(T);
unsafe impl<T> Send for AssertSend<T> {}

impl<T> Foreign<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner: ManuallyDrop::new(inner),
            origin_shard: this_shard(),
        }
    }
}

impl<T: 'static> Drop for Foreign<T> {
    fn drop(&mut self) {
        let inner = unsafe { ManuallyDrop::take(&mut self.inner) };
        if this_shard() != self.origin_shard {
            // Need to drop it on a foreign shard
            let inner = AssertSend(inner);
            let _ = submit_to(self.origin_shard, move || async { std::mem::drop(inner) });
        }
    }
}

pub fn submit_to<Func, Fut, Ret>(shard_id: u32, func: Func) -> impl Future<Output = Ret>
where
    Func: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Ret> + 'static,
    Ret: Send + 'static,
{
    async { todo!() }
}
mod ffi {
    extern "C" {
        pub(super) fn shard_count() -> u32;
        pub(super) fn this_shard() -> u32;
    }
}

fn shard_count() -> u32 {
    unsafe { ffi::shard_count() }
}

fn this_shard() -> u32 {
    unsafe { ffi::this_shard() }
}
