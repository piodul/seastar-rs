use std::cell::UnsafeCell;
use std::error::Error;
use std::future::Future;
use std::mem::MaybeUninit;
use std::rc::Rc;
use std::sync::Arc;

use futures::FutureExt;

mod ffi {
    extern "C" {
        pub(super) fn shard_count() -> u32;
        pub(super) fn this_shard() -> u32;
    }
}

fn shard_count() -> u32 {
    unsafe { ffi::shard_count() }
}

// Beware of unsafe code here!
// We heavily depend on submit_to here...

struct ShardedStorage<T> {
    shards: Box<[UnsafeCell<MaybeUninit<T>>]>,
}

impl<T> ShardedStorage<T>
where
    T: Send,
{
    fn new_from_locally_constructed(shards: Vec<T>) -> Self {
        assert_eq!(shards.len(), shard_count() as usize);
        unsafe {
            Self {
                shards: std::mem::transmute(shards.into_boxed_slice()),
            }
        }
    }
}

impl<T> ShardedStorage<T> {
    fn new_uninitialized() -> Self {
        Self {
            shards: std::iter::repeat_with(|| UnsafeCell::new(MaybeUninit::uninit()))
                .take(shard_count() as usize)
                .collect(),
        }
    }

    // Unsafe: you must guarantee that there are no immutable references
    // to the local shard, and that the object was not initialized before.
    unsafe fn initialize(&self, shard: u32, v: T) {
        (*self.shards.get_unchecked(shard as usize).get()).write(v);
    }

    // Unsafe: you must guarantee that there are no immutable references
    // to the local shard and that the object was initialized.
    unsafe fn release_local(&self, shard: u32) {
        (*self.shards.get_unchecked(shard as usize).get()).assume_init_drop();
    }

    // Not unsafe because it requires mutable access You should make sure that
    // all shards were destroyed, otherwise a memory leak could occur.
    fn release_slice(&mut self) {
        self.shards = Box::new([])
    }

    // Unsafe: assumes that the value was initialized already on this shard.
    unsafe fn get(&self, shard: u32) -> &T {
        unsafe {
            let slot_ref = &*self.shards.get_unchecked(shard as usize).get();
            slot_ref.assume_init_ref()
        }
    }
}

impl<T> Drop for ShardedStorage<T> {
    fn drop(&mut self) {
        assert!(self.shards.is_empty());
    }
}

pub struct Sharded<T> {
    storage: ShardedStorage<T>,
}

// impl<T> Sharded<T> where T: Send {
//     pub fn new_from_locally_constructed(shards: Vec<T>)
// }

// impl<T> Sharded<T> {
//     pub async fn new<C, E, Fut>(constructor: C) -> Result<C, E>
//     where
//         C: FnOnce() -> Fut + Send + Clone + 'static,
//         Fut: Future<Output = Result<T, E>> + 'static,
//         E: Send + 'static,
//     {
//         // TODO: For increased safety, this should run in a separate task
//         let storage = ShardedStorage::new();
//         let results = invoke_on_all_shards(|| async {
//             match constructor().await {
//                 Ok(t) => {
//                     unsafe { storage.initialize_local(t) };
//                     None
//                 }
//                 Err(err) => Some(err),
//             }
//         });
//     }

//     pub async fn dispose(self) {}
// }

async fn try_invoke_on_all_shards<Func, Fut, Ret, E>(f: Func) -> Result<Vec<Ret>, E>
where
    Func: FnOnce() -> Fut + Send + Clone + 'static,
    Fut: Future<Output = Result<Ret, E>> + 'static,
    Ret: Send + 'static,
    E: Send + 'static,
{
    // The futures returned from submit_to do not need to be polled in order
    // for them to make progress, so we don't have to use join_all or other
    // stuff like that.

    // TODO: This could probably be optimized: tasks spawned with submit_to
    // are guaranteed to be polled, so we could avoid the need for Arc

    let futs: Vec<_> = (0..shard_count())
        .map(|s| {
            let f = f.clone();
            submit_to(s, move || f())
        })
        .collect();
    let mut ret = Vec::with_capacity(shard_count() as usize);
    let mut futs = futs.into_iter();
    while let Some(f) = futs.next() {
        match f.await {
            Ok(v) => ret.push(v),
            Err(err) => {
                // Wait for other futures, then return
                for f in futs {
                    // Ignore errors
                    let _ = f.await;
                }
                return Err(err);
            }
        }
    }
    Ok(ret)
}

// Uninhabitable type
enum Never {}

async fn invoke_on_all_shards<Func, Fut, Ret>(f: Func) -> Vec<Ret>
where
    Func: FnOnce() -> Fut + Send + Clone + 'static,
    Fut: Future<Output = Ret> + 'static,
    Ret: Send + 'static,
{
    // TODO: It seems that using an irrefutable pattern does not work,
    // although in theory it should
    //
    // let Ok(ret) = ...
    match try_invoke_on_all_shards(move || f().map(Ok::<Ret, Never>)).await {
        Ok(ret) => ret,
        Err(_) => unreachable!(),
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
