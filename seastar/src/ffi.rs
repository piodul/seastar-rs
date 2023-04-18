use std::cell::Cell;
use std::ffi::{c_int, c_void};
use std::future::Future;
use std::future::IntoFuture;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

unsafe trait FfiType: Sized + 'static {
    fn spawn_fut<F>(fut: F)
    where
        F: Future<Output = Self> + 'static;
}

#[repr(C)]
struct CppFfiTypeVtable {
    satisfy_promise: unsafe extern "C" fn(*mut c_void, *mut c_void),
}

macro_rules! unsafe_define_ffi_type {
    ($label:ident, $typ:ty) => {
        extern "C" fn

        unsafe impl FfiType for $typ {
            fn spawn_fut<F>(fut: F)
            where
                F: Future<Output = Self> + 'static,
            {

            }
        }
    };
}

#[repr(C)]
struct CppFutureWrapperVtable {
    poll_fn: unsafe extern "C" fn(*const c_void, *mut c_void) -> c_int,
    attach_wait_state: unsafe extern "C" fn(*const c_void, *const c_void),
    destroy: unsafe extern "C" fn(*const c_void),
}

struct WaitState {
    waker: Cell<Option<Waker>>,
}

pub unsafe extern "C" fn seastar_rs_wait_state_wake_and_detach(wait_state: *const c_void) {
    let wait_state = Rc::from_raw(wait_state as *const WaitState);
    if let Some(waker) = wait_state.waker.take() {
        waker.wake();
    }
}

// extern "C" pub fn

trait SeastarRsFuture: IntoFuture {
    /// Spawns given Rust future as a task on the Seastar runtime.
    fn spawn(fut: impl Future<Output = Self::Output>) -> Self::IntoFuture;
}

#[repr(C)]
struct SeastarFutureWrapper {
    ptr: *const c_void,
}

struct ActualSeastarFuture {
    ptr: *const c_void,
    vtable: &'static CppFutureWrapperVtable,
    wait_state: Option<Rc<WaitState>>, // TODO: Rc unnecessarily has both weak and strong counts
}

impl Future for ActualSeastarFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY:
        // - self.ptr points to a valid future of an appropriate type
        //   (this should be guaranteed by seastar-rs bindings)
        // - if poll_fn returns 1, then out_storage will contain a valid value
        unsafe {
            let mut out_storage = MaybeUninit::uninit();
            if (self.vtable.poll_fn)(self.ptr, out_storage.as_mut_ptr() as *mut c_void) == 1 {
                Poll::Ready(out_storage.assume_init())
            } else {
                match &mut self.wait_state {
                    Some(wait_state) => wait_state.waker.set(Some(cx.waker().clone())),
                    None => {
                        let wait_state = Rc::new(WaitState {
                            waker: Cell::new(Some(cx.waker().clone())),
                        });
                        (self.vtable.attach_wait_state)(
                            self.ptr,
                            Rc::into_raw(wait_state.clone()) as *const c_void,
                        );
                    }
                }
                Poll::Pending
            }
        }

        // let waker = unsafe { Waker::from_raw(RawWaker::new(self.ptr, &RAW_WAKER_VTABLE)) };
    }
}

static RAW_WAKER_VTABLE: RawWakerVTable =
    RawWakerVTable::new(waker_clone, waker_wake, waker_wake_by_ref, waker_dispose);

unsafe fn waker_clone(data: *const ()) -> RawWaker {
    seastar_rs_waker_clone(data);
    RawWaker::new(data, &RAW_WAKER_VTABLE)
}

unsafe fn waker_wake(data: *const ()) {
    seastar_rs_waker_wake(data);
}

unsafe fn waker_wake_by_ref(data: *const ()) {
    seastar_rs_waker_wake_by_ref(data);
}

unsafe fn waker_dispose(data: *const ()) {
    seastar_rs_waker_dispose(data);
}

extern "C" {
    fn seastar_rs_waker_clone(data: *const ());
    fn seastar_rs_waker_wake(data: *const ());
    fn seastar_rs_waker_wake_by_ref(data: *const ());
    fn seastar_rs_waker_dispose(data: *const ());
}
