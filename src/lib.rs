use rand::prelude::SliceRandom;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Wake, Waker};

type Task = Pin<Box<dyn Future<Output = ()>>>;

struct TaskWaker {
    task: Rc<RefCell<Option<Task>>>,
}

// TODO:
unsafe impl Sync for TaskWaker {}
unsafe impl Send for TaskWaker {}

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
pub struct Executor {
    queue: RefCell<VecDeque<Task>>,
}

mod current_thread {
    use crate::Executor;
    use std::cell::OnceCell;
    use std::rc::Rc;

    thread_local! {
        pub(super) static EXECUTOR: OnceCell<Rc<Executor>> = OnceCell::new();
    }
}

pub fn spawn(fut: impl Future<Output = ()> + 'static) {
    // TODO: unwrap
    current_thread::EXECUTOR.with(|e| e.get().unwrap().spawn(fut));
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

    pub fn block_on<Out: 'static>(
        self: &Rc<Self>,
        fut: impl Future<Output = Out> + 'static,
    ) -> Out {
        self.register();

        let mut rng = rand::thread_rng();

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

            // TODO: unwrap
            let mut fut = self.queue.borrow_mut().pop_front().unwrap();

            if fut.as_mut().poll(&mut ctx).is_pending() {
                self.queue.borrow_mut().push_back(fut);
                continue;
            }

            if let Some(output) = output.borrow_mut().take() {
                break output;
            }
        }
    }
}
