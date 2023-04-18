use futures::channel::oneshot;
use futures::{Future, FutureExt};
use std::ffi;
use std::mem::ManuallyDrop;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

type RustFuturePollFn = unsafe extern "C" fn(*mut ffi::c_void, *mut ffi::c_void) -> ffi::c_int;

extern "C" {
    fn seastar_rs_waker_clone(task_ptr: *mut ffi::c_void);
    fn seastar_rs_waker_wake(task_ptr: *mut ffi::c_void);
    fn seastar_rs_waker_wake_by_ref(task_ptr: *mut ffi::c_void);
    fn seastar_rs_waker_dispose(task_ptr: *mut ffi::c_void);
}

extern "C" {
    fn seastar_rs_spawn(poll_fn: RustFuturePollFn, rust_future: *mut ffi::c_void);
    fn seastar_rs_submit_to(
        poll_fn: RustFuturePollFn,
        rust_future: *mut ffi::c_void,
        shard: ffi::c_uint,
    );
}

static WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    |ptr| unsafe {
        seastar_rs_waker_clone(ptr as *mut ffi::c_void);
        make_waker(ptr)
    },
    |ptr| unsafe { seastar_rs_waker_wake(ptr as *mut ffi::c_void) },
    |ptr| unsafe { seastar_rs_waker_wake_by_ref(ptr as *mut ffi::c_void) },
    |ptr| unsafe { seastar_rs_waker_dispose(ptr as *mut ffi::c_void) },
);

unsafe fn make_waker(task: *const ()) -> RawWaker {
    RawWaker::new(task, &WAKER_VTABLE)
}

unsafe extern "C" fn poll_fn<Fut>(task: *mut ffi::c_void, fut: *mut ffi::c_void) -> ffi::c_int
where
    Fut: Future<Output = ()> + 'static,
{
    let raw = make_waker(task as *const ());

    // Dropped waker will decrement the reference count. We want to
    // prevent that as we didn't increment it when creating the waker.
    let waker = ManuallyDrop::new(Waker::from_raw(raw));
    let mut context = Context::from_waker(&waker);

    // TODO: Handle panics
    match Pin::new_unchecked(&mut *(fut as *mut Fut)).poll(&mut context) {
        Poll::Pending => 0,
        Poll::Ready(()) => {
            // Drop the future, then return
            let _ = Box::from_raw(fut);
            1
        }
    }
}

fn get_poll_fn<Fut>(_f: &Fut) -> RustFuturePollFn
where
    Fut: Future<Output = ()> + 'static,
{
    poll_fn::<Fut>
}

pub(crate) fn spawn_impl<Fut, Ret>(fut: Fut) -> impl Future<Output = Ret>
where
    Fut: Future<Output = Ret> + 'static,
    Ret: 'static,
{
    // TODO: Use a non-thread-safe version of the channel
    let (sender, receiver) = oneshot::channel();
    let fut = async move {
        let _ = sender.send(AssertUnwindSafe(fut).catch_unwind().await);
    };
    let poll_fn = get_poll_fn(&fut);
    let fut_ptr = Box::into_raw(Box::new(fut)) as *mut ffi::c_void;
    unsafe {
        seastar_rs_spawn(poll_fn, fut_ptr);
    }
    async move {
        match receiver.await.unwrap() {
            Ok(v) => v,
            Err(err) => std::panic::resume_unwind(err),
        }
    }
}

pub(crate) fn submit_to_impl<Func, Fut, Ret>(shard: u32, func: Func) -> impl Future<Output = Ret>
where
    Func: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Ret> + 'static,
    Ret: Send + 'static,
{
    let (sender, receiver) = oneshot::channel();
    let fut = async move {
        let fut = func();
        let _ = sender.send(AssertUnwindSafe(fut).catch_unwind().await);
    };
    let poll_fn = get_poll_fn(&fut);
    let fut_ptr = Box::into_raw(Box::new(fut)) as *mut ffi::c_void;
    unsafe {
        seastar_rs_submit_to(poll_fn, fut_ptr, shard as ffi::c_uint);
    }
    async move {
        match receiver.await.unwrap() {
            Ok(v) => v,
            Err(err) => std::panic::resume_unwind(err),
        }
    }
}
