// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

use crate::threading::sync::oneshot;
use crate::threading::task::{JoinError, JoinHandle};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::thread::{self, Thread};

thread_local! {
    static CURRENT_HANDLE: RefCell<Option<Handle>> = const { RefCell::new(None) };
}

static GLOBAL_POOL: OnceLock<Arc<WorkerPool>> = OnceLock::new();

fn get_global_pool() -> Arc<WorkerPool> {
    GLOBAL_POOL
        .get_or_init(|| {
            let pool = Arc::new(WorkerPool::new(4, "arkcode-worker"));
            pool.start_workers();
            pool
        })
        .clone()
}

fn thread_waker(thread: Thread) -> Waker {
    let raw = RawWaker::new(
        Box::into_raw(Box::new(thread)) as *const (),
        &THREAD_WAKER_VTABLE,
    );
    unsafe { Waker::from_raw(raw) }
}

static THREAD_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    |ptr| {
        let thread = unsafe { &*(ptr as *const Thread) };
        RawWaker::new(
            Box::into_raw(Box::new(thread.clone())) as *const (),
            &THREAD_WAKER_VTABLE,
        )
    },
    |ptr| {
        let thread = unsafe { Box::from_raw(ptr as *mut Thread) };
        thread.unpark();
    },
    |ptr| {
        let thread = unsafe { &*(ptr as *const Thread) };
        thread.unpark();
    },
    |ptr| {
        let _ = unsafe { Box::from_raw(ptr as *mut Thread) };
    },
);

struct TaskInner {
    future: Mutex<Option<Pin<Box<dyn Future<Output = ()> + Send>>>>,
    pool: Arc<WorkerPool>,
    aborted: AtomicBool,
}

impl TaskInner {
    fn schedule(self: Arc<Self>) {
        if self.aborted.load(Ordering::SeqCst) {
            return;
        }
        let pool = Arc::clone(&self.pool);
        pool.enqueue_task(self);
    }

    fn run(self: Arc<Self>) {
        if self.aborted.load(Ordering::SeqCst) {
            *self.future.lock().unwrap() = None;
            return;
        }

        let mut future_guard = self.future.lock().unwrap();
        if let Some(ref mut fut) = *future_guard {
            let waker = task_waker(&self);
            let mut cx = Context::from_waker(&waker);
            let poll_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                fut.as_mut().poll(&mut cx)
            }));
            match poll_res {
                Ok(Poll::Ready(())) => {
                    *future_guard = None;
                }
                Ok(Poll::Pending) => {}
                Err(_) => {
                    *future_guard = None;
                }
            }
        }
    }
}

fn task_waker(task: &Arc<TaskInner>) -> Waker {
    let raw = RawWaker::new(
        Arc::into_raw(Arc::clone(task)) as *const (),
        &TASK_WAKER_VTABLE,
    );
    unsafe { Waker::from_raw(raw) }
}

static TASK_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    |ptr| {
        let arc = unsafe { Arc::from_raw(ptr as *const TaskInner) };
        let clone = Arc::clone(&arc);
        let _ = Arc::into_raw(arc);
        RawWaker::new(Arc::into_raw(clone) as *const (), &TASK_WAKER_VTABLE)
    },
    |ptr| {
        let arc = unsafe { Arc::from_raw(ptr as *const TaskInner) };
        arc.schedule();
    },
    |ptr| {
        let arc = unsafe { Arc::from_raw(ptr as *const TaskInner) };
        let clone = Arc::clone(&arc);
        let _ = Arc::into_raw(arc);
        clone.schedule();
    },
    |ptr| {
        let _ = unsafe { Arc::from_raw(ptr as *const TaskInner) };
    },
);

struct WorkerPool {
    threads: usize,
    name: String,
    queue: Mutex<VecDeque<Arc<TaskInner>>>,
    condvar: Condvar,
    shutdown: AtomicBool,
}

impl WorkerPool {
    fn new(threads: usize, name: &str) -> Self {
        WorkerPool {
            threads: threads.max(1),
            name: name.to_string(),
            queue: Mutex::new(VecDeque::new()),
            condvar: Condvar::new(),
            shutdown: AtomicBool::new(false),
        }
    }

    fn start_workers(self: &Arc<Self>) {
        for i in 0..self.threads {
            let pool = Arc::clone(self);
            let name = format!("{}-{}", self.name, i);
            thread::Builder::new()
                .name(name)
                .spawn(move || loop {
                    let task = {
                        let mut guard = pool.queue.lock().unwrap();
                        while guard.is_empty() && !pool.shutdown.load(Ordering::SeqCst) {
                            guard = pool.condvar.wait(guard).unwrap();
                        }
                        if pool.shutdown.load(Ordering::SeqCst) && guard.is_empty() {
                            break;
                        }
                        guard.pop_front()
                    };

                    if let Some(task) = task {
                        task.run();
                    }
                })
                .expect("Failed to spawn runtime worker thread");
        }
    }

    fn enqueue_task(&self, task: Arc<TaskInner>) {
        let mut guard = self.queue.lock().unwrap();
        guard.push_back(task);
        self.condvar.notify_one();
    }
}

#[derive(Clone)]
pub struct Handle {
    pool: Arc<WorkerPool>,
}

impl Handle {
    pub fn current() -> Handle {
        Self::try_current().unwrap_or_else(|_| Handle {
            pool: get_global_pool(),
        })
    }

    pub fn try_current() -> Result<Handle, TryCurrentError> {
        CURRENT_HANDLE.with(|cell| {
            if let Some(ref h) = *cell.borrow() {
                Ok(h.clone())
            } else {
                Err(TryCurrentError(()))
            }
        })
    }

    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let aborted = Arc::new(AtomicBool::new(false));
        let aborted_clone = Arc::clone(&aborted);

        let wrapped = async move {
            let val = future.await;
            let _ = tx.send(Ok(val));
        };

        let task = Arc::new(TaskInner {
            future: Mutex::new(Some(Box::pin(wrapped))),
            pool: Arc::clone(&self.pool),
            aborted: AtomicBool::new(false),
        });

        self.pool.enqueue_task(task);

        JoinHandle {
            rx,
            aborted: aborted_clone,
        }
    }

    pub fn spawn_blocking<F, R>(&self, f: F) -> JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let aborted = Arc::new(AtomicBool::new(false));

        thread::Builder::new()
            .name("arkcode-blocking".into())
            .spawn(move || {
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
                let _ = match res {
                    Ok(val) => tx.send(Ok(val)),
                    Err(_) => tx.send(Err(JoinError::panic())),
                };
            })
            .expect("Failed to spawn blocking thread");

        JoinHandle { rx, aborted }
    }

    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        let _guard = self.enter();
        let thread = thread::current();
        let waker = thread_waker(thread);
        let mut cx = Context::from_waker(&waker);
        let mut pinned = std::pin::pin!(future);

        loop {
            match pinned.as_mut().poll(&mut cx) {
                Poll::Ready(res) => return res,
                Poll::Pending => thread::park(),
            }
        }
    }

    pub fn enter(&self) -> EnterGuard<'_> {
        CURRENT_HANDLE.with(|cell| {
            let prev = cell.borrow_mut().replace(self.clone());
            EnterGuard {
                _handle: self,
                prev,
            }
        })
    }
}

pub struct EnterGuard<'a> {
    _handle: &'a Handle,
    prev: Option<Handle>,
}

impl<'a> Drop for EnterGuard<'a> {
    fn drop(&mut self) {
        CURRENT_HANDLE.with(|cell| {
            *cell.borrow_mut() = self.prev.take();
        });
    }
}

#[derive(Debug)]
pub struct TryCurrentError(());

impl fmt::Display for TryCurrentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "no threading runtime active on this thread")
    }
}

impl std::error::Error for TryCurrentError {}

pub struct Runtime {
    handle: Handle,
}

impl Runtime {
    pub fn new() -> io::Result<Runtime> {
        Builder::new_multi_thread().build()
    }

    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.handle.block_on(future)
    }

    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.handle.spawn(future)
    }
}

pub struct Builder {
    worker_threads: usize,
    thread_name: String,
}

impl Builder {
    pub fn new_multi_thread() -> Builder {
        let cpus = thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        Builder {
            worker_threads: cpus.max(2),
            thread_name: "arkcode-runtime-worker".into(),
        }
    }

    pub fn build(&mut self) -> io::Result<Runtime> {
        let pool = Arc::new(WorkerPool::new(self.worker_threads, &self.thread_name));
        pool.start_workers();
        Ok(Runtime {
            handle: Handle { pool },
        })
    }
}
