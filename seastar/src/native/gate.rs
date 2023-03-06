use std::cell::Cell;
use std::error::Error;
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;
use std::task::{Poll, Waker};

#[derive(Default)]
pub struct Gate<T> {
    inner: T,

    use_count: Cell<usize>,
    closed: Cell<bool>,
    waker: Cell<Option<Waker>>,
}

impl<T> Gate<T> {
    #[inline]
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            use_count: Cell::new(0),
            closed: Cell::new(false),
            waker: Cell::new(None),
        }
    }

    #[inline]
    pub fn is_closed(&self) -> bool {
        self.closed.get()
    }

    #[inline]
    pub fn use_count(&self) -> usize {
        self.use_count.get()
    }

    #[inline]
    pub fn enter(&self) -> Result<GateHolder<T>, GateClosedError> {
        if !self.is_closed() {
            self.add_ref();
            Ok(GenericGateHolder { gate: self })
        } else {
            Err(GateClosedError)
        }
    }

    #[inline]
    pub fn enter_owned(self: &Rc<Self>) -> Result<OwnedGateHolder<T>, GateClosedError> {
        if !self.is_closed() {
            self.add_ref();
            Ok(GenericGateHolder {
                gate: Rc::clone(self),
            })
        } else {
            Err(GateClosedError)
        }
    }

    #[inline]
    pub async fn close(&self) {
        if self.is_closed() {
            panic!("attempted to close the gate for the second time");
        }

        self.closed.set(true);

        std::future::poll_fn(|cx| {
            if self.use_count.get() == 0 {
                Poll::Ready(())
            } else {
                self.waker.set(Some(cx.waker().clone()));
                Poll::Pending
            }
        })
        .await
    }

    fn add_ref(&self) {
        self.use_count.set(self.use_count.get() + 1);
    }

    fn dec_ref(&self) {
        let new_count = self.use_count.get() - 1;
        self.use_count.set(new_count);
        if self.is_closed() && new_count == 0 {
            // The gate was closed but there were some references to it,
            // therefore the close() future stored a waker and it's safe
            // to unwrap here.
            self.waker.take().unwrap().wake();
        }
    }
}

pub type GateHolder<'g, T> = GenericGateHolder<&'g Gate<T>, T>;
pub type OwnedGateHolder<T> = GenericGateHolder<Rc<Gate<T>>, T>;

pub struct GenericGateHolder<P, T>
where
    P: Deref<Target = Gate<T>>,
{
    gate: P,
}

impl<P, T> Deref for GenericGateHolder<P, T>
where
    P: Deref<Target = Gate<T>>,
{
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.gate.inner
    }
}

impl<P, T> Drop for GenericGateHolder<P, T>
where
    P: Deref<Target = Gate<T>>,
{
    #[inline]
    fn drop(&mut self) {
        self.gate.dec_ref();
    }
}

#[derive(Debug)]
pub struct GateClosedError;

impl fmt::Display for GateClosedError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "attempted to enter a closed gate")
    }
}

impl Error for GateClosedError {}
