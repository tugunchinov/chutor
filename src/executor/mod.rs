mod handle;

use rand::prelude::SliceRandom;
use rand::SeedableRng;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

pub fn run_with_seed<Out: 'static>(fut: impl Future<Output = Out> + 'static, seed: u64) -> Out {
    let executor = Executor::new();
    executor.block_on(fut, Some(seed))
}

pub fn spawn(fut: impl Future<Output = ()> + 'static) {
    // TODO: unwrap
    current_thread::EXECUTOR.with(|e| e.get().unwrap().spawn(fut));
}

type Task = Pin<Box<dyn Future<Output = ()> + 'static>>;

struct TaskWaker {
    task: Rc<RefCell<Option<Task>>>,
}

// TODO:
unsafe impl Sync for TaskWaker {}
unsafe impl Send for TaskWaker {}

// TODO: separate module
impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        if let Some(task) = self.task.borrow_mut().take() {
            // TODO: unwrap
            current_thread::EXECUTOR.with(|e| {
                e.get().unwrap().queue.borrow_mut().push_back(task);
            });
        }
    }
}

#[derive(Default)]
struct Executor {
    queue: RefCell<VecDeque<Task>>,
}

mod current_thread {
    use crate::executor::Executor;
    use std::cell::OnceCell;
    use std::rc::Rc;

    thread_local! {
        pub(super) static EXECUTOR: OnceCell<Rc<Executor>> = const { OnceCell::new() };
    }
}

struct DummyF {
    start: Instant,
}

impl Future for DummyF {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.start + Duration::from_secs(3) > Instant::now() {
            println!("DUMMY PENDING");
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            println!("DUMMY READY");
            Poll::Ready(())
        }
    }
}

impl Executor {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            queue: RefCell::new(VecDeque::new()),
        })
    }

    fn register(self: &Rc<Self>) {
        // TODO: unwrap
        let _ = current_thread::EXECUTOR.with(|e| e.set(self.clone()));
    }

    fn spawn(&self, fut: impl Future<Output = ()> + 'static) {
        self.queue.borrow_mut().push_back(Box::pin(fut));
    }

    fn block_on<Out: 'static>(
        self: &Rc<Self>,
        fut: impl Future<Output = Out> + 'static,
        seed: Option<u64>,
    ) -> Out {
        self.spawn(DummyF {
            start: Instant::now(),
        });

        self.register();

        let seed = seed.unwrap_or(rand::random());

        // TODO: use log
        println!("running with seed: {seed}");

        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

        let output: Rc<RefCell<Option<Out>>> = Rc::new(RefCell::new(None));

        {
            let output = Rc::clone(&output);

            self.spawn(async move {
                *output.borrow_mut() = Some(fut.await);
            });
        }

        let task_waker = Arc::new(TaskWaker {
            task: Rc::new(RefCell::new(None)),
        });

        let waker = Waker::from(Arc::clone(&task_waker));
        let mut ctx = Context::from_waker(&waker);

        loop {
            self.queue.borrow_mut().make_contiguous().shuffle(&mut rng);

            let Some(mut fut) = self.queue.borrow_mut().pop_front() else {
                // libc::syscall();
                // implement via futex
                // std::thread::park();
                println!("No tasks to run");
                continue;
            };

            if fut.as_mut().poll(&mut ctx).is_pending() {
                *task_waker.task.borrow_mut() = Some(fut);
            }

            if let Some(output) = output.borrow_mut().take() {
                break output;
            }
        }
    }
}
