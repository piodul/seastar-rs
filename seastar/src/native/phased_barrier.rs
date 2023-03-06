use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::task::{Poll, Waker};

#[derive(Default)]
pub struct PhasedBarrier {
    current_stage: RefCell<Rc<PhasedBarrierStage>>,
}

#[derive(Default)]
struct PhasedBarrierStage {
    // When the stage is dropped, it will notify the waiting future.
    wake_on_drop: WakeOnDrop,

    // Prevents the next stage from being considered finished until this
    // stage is finished.
    //
    // Note that a chain of stages linked by the `next_stage_holder` field
    // can get potentially quite long. In order to prevent stack overflow
    // and stalls, we are performing gentle cleanup.
    next_stage_holder: Cell<Option<Rc<PhasedBarrierStage>>>,
}

impl Drop for PhasedBarrierStage {
    #[inline]
    fn drop(&mut self) {
        let holder = self.next_stage_holder.take();
        crate::gentle_cleanup(holder, |holder| {
            match Rc::try_unwrap(holder) {
                Err(_ptr) => {
                    // Somebody is still using that stage. Somebody else
                    // will be responsible for cleanup from now on.
                    None
                }
                Ok(stage) => stage.next_stage_holder.take(),
            }
        });
    }
}

impl PhasedBarrier {
    /// Creates a new phased barrier.
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }

    /// Start a new operation.
    #[inline]
    pub fn start(&self) -> Operation {
        Operation(Rc::clone(&*self.current_stage.borrow()))
    }

    /// Starts a new phase immediately.
    ///
    /// Returns a future that will resolve after all previous operations
    /// have finished - but it's fine to just drop that future.
    pub fn advance(&self) -> impl Future<Output = ()> {
        // Create a new stage and swap it with the current one
        let new_stage = Default::default();
        let old_stage = self.current_stage.replace(Rc::clone(&new_stage));

        // Place a shared pointer to the new stage in the old stage.
        // We must make sure that a newer stage finished only after previous
        // stages have finished. Because our future gets notified when the
        // stage object is dropped, making the older stage keep a newer stage
        // alive takes care of this problem.
        old_stage.next_stage_holder.set(Some(new_stage));

        // Degrade the old_stage pointer to a weak pointer. We want
        // the returned future have access to it, but not keep it alive
        // longer than necessary.
        let old_stage = Rc::downgrade(&old_stage);

        std::future::poll_fn(move |cx| {
            match old_stage.upgrade() {
                Some(stage) => {
                    // Stage still alive - register the waker
                    stage.wake_on_drop.0.set(Some(cx.waker().clone()));
                    Poll::Pending
                }
                None => {
                    // All references to the stage have dropped, so it can
                    // be considered finished.
                    Poll::Ready(())
                }
            }
        })
    }

    /// Returns the number of operations in progress in the current stage.
    #[inline]
    pub fn operations_in_progress(&self) -> usize {
        Rc::strong_count(&*self.current_stage.borrow()) - 1
    }
}

/// Represents an ongoing operation.
pub struct Operation(Rc<PhasedBarrierStage>);

#[derive(Default)]
struct WakeOnDrop(Cell<Option<Waker>>);

impl Drop for WakeOnDrop {
    #[inline]
    fn drop(&mut self) {
        if let Some(waker) = self.0.take() {
            waker.wake();
        }
    }
}
