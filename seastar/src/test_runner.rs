use futures::channel::{mpsc, oneshot};
use futures::{Future, FutureExt, SinkExt, StreamExt};
use std::panic::{resume_unwind, AssertUnwindSafe, UnwindSafe};
use std::pin::Pin;
use std::sync::{Mutex, PoisonError};
use std::thread::JoinHandle;

use crate::AppTemplate;

struct TestCase {
    fun: Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()> + UnwindSafe + 'static>> + Send>,
    result_sender: oneshot::Sender<std::thread::Result<()>>,
}

struct SeastarTestRunner {
    test_sender: mpsc::Sender<TestCase>,
    join_handle: JoinHandle<()>,
}

static TEST_RUNNER: Mutex<Option<SeastarTestRunner>> = Mutex::new(None);

impl SeastarTestRunner {
    fn start() -> SeastarTestRunner {
        let (test_sender, mut test_receiver) = mpsc::channel::<TestCase>(1);
        let join_handle = std::thread::spawn(move || {
            AppTemplate::default().run_void(std::env::args().take(1), async move {
                // We will block while waiting for the next test, but that's ok.
                // After receiving a test we poll it to completion, then wait
                // for the next test. There isn't anything useful to do in the
                // meantime.
                //
                // This doesn't allow for multiple concurrent tests yet,
                // unfortunately.
                while let Some(tc) = futures::executor::block_on(test_receiver.next()) {
                    let res = (tc.fun)().catch_unwind().await;
                    let _ = tc.result_sender.send(res);
                }
                Ok(())
            });
        });

        SeastarTestRunner {
            test_sender,
            join_handle,
        }
    }

    fn stop(self) {
        std::mem::drop(self.test_sender);
        self.join_handle.join().unwrap();
    }

    fn run_test<T, F>(&mut self, test: T) -> std::thread::Result<()>
    where
        T: FnOnce() -> F,
        T: Send + 'static,
        F: Future<Output = ()> + 'static,
    {
        let (sender, receiver) = oneshot::channel();
        let fun = Box::new(move || {
            Box::pin(AssertUnwindSafe(test()))
                as Pin<Box<dyn Future<Output = ()> + UnwindSafe + 'static>>
        });
        let tc = TestCase {
            fun,
            result_sender: sender,
        };
        futures::executor::block_on(self.test_sender.send(tc)).unwrap();
        futures::executor::block_on(receiver.map(Result::unwrap))
    }
}

pub fn run_test<T, F>(test: T)
where
    T: FnOnce() -> F,
    T: Send + 'static,
    F: Future<Output = ()> + 'static,
{
    let mut lock = TEST_RUNNER.lock().unwrap();
    let need_init = lock.is_none();
    let runtime = lock.get_or_insert_with(SeastarTestRunner::start);
    if need_init {
        unsafe {
            libc::atexit(stop_runner);
        }
    }
    let res = runtime.run_test(test);
    // Drop the lock now, because we might rethrow the panic
    std::mem::drop(lock);
    if let Err(p) = res {
        resume_unwind(p);
    }
}

extern "C" fn stop_runner() {
    let runner = TEST_RUNNER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    if let Some(runner) = runner {
        runner.stop();
    }
}

#[test]
fn test_test_runner() {
    run_test(|| async {
        println!("Hello from seastar!");
    })
}
