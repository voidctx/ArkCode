// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll, Waker};
use std::thread;
pub use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Elapsed(());

impl Elapsed {
    pub(crate) fn new() -> Self {
        Elapsed(())
    }
}

impl fmt::Display for Elapsed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "deadline has elapsed")
    }
}

impl std::error::Error for Elapsed {}

struct TimerEntry {
    deadline: Instant,
    id: u64,
    waker: Waker,
}

impl PartialEq for TimerEntry {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline && self.id == other.id
    }
}

impl Eq for TimerEntry {}

impl PartialOrd for TimerEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimerEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.deadline.cmp(&other.deadline)
    }
}

struct TimerState {
    heap: BinaryHeap<Reverse<TimerEntry>>,
}

static TIMER_STATE: OnceLock<Arc<Mutex<TimerState>>> = OnceLock::new();
static NEXT_TIMER_ID: AtomicU64 = AtomicU64::new(1);

fn get_timer_state() -> &'static Arc<Mutex<TimerState>> {
    TIMER_STATE.get_or_init(|| {
        let state = Arc::new(Mutex::new(TimerState {
            heap: BinaryHeap::new(),
        }));

        let state_clone = Arc::clone(&state);
        thread::Builder::new()
            .name("threading-timer".into())
            .spawn(move || loop {
                let now = Instant::now();
                let mut next_sleep = Duration::from_millis(50);
                let mut to_wake = Vec::new();

                {
                    let mut guard = state_clone.lock().unwrap();
                    while let Some(Reverse(entry)) = guard.heap.peek() {
                        if entry.deadline <= now {
                            let entry = guard.heap.pop().unwrap().0;
                            to_wake.push(entry.waker);
                        } else {
                            next_sleep = entry.deadline.saturating_duration_since(now).min(Duration::from_millis(50));
                            break;
                        }
                    }
                }

                for waker in to_wake {
                    waker.wake();
                }

                thread::sleep(next_sleep.max(Duration::from_millis(1)));
            })
            .expect("Failed to spawn timer thread");

        state
    })
}

pub fn sleep(duration: Duration) -> Sleep {
    sleep_until(Instant::now() + duration)
}

pub fn sleep_until(deadline: Instant) -> Sleep {
    Sleep {
        deadline,
        id: NEXT_TIMER_ID.fetch_add(1, Ordering::Relaxed),
        _pin: std::marker::PhantomPinned,
    }
}

#[derive(Debug)]
pub struct Sleep {
    deadline: Instant,
    id: u64,
    _pin: std::marker::PhantomPinned,
}

impl Sleep {
    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    pub fn is_elapsed(&self) -> bool {
        Instant::now() >= self.deadline
    }

    pub fn reset(self: Pin<&mut Self>, deadline: Instant) {
        unsafe {
            self.get_unchecked_mut().deadline = deadline;
        }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if Instant::now() >= self.deadline {
            return Poll::Ready(());
        }

        let state = get_timer_state();
        let mut guard = state.lock().unwrap();
        guard.heap.push(Reverse(TimerEntry {
            deadline: self.deadline,
            id: self.id,
            waker: cx.waker().clone(),
        }));

        Poll::Pending
    }
}

pub fn timeout<F: Future>(duration: Duration, future: F) -> Timeout<F> {
    timeout_at(Instant::now() + duration, future)
}

pub fn timeout_at<F: Future>(deadline: Instant, future: F) -> Timeout<F> {
    Timeout {
        future,
        sleep: sleep_until(deadline),
    }
}

pub struct Timeout<F> {
    future: F,
    sleep: Sleep,
}

impl<F: Future> Future for Timeout<F> {
    type Output = Result<F::Output, Elapsed>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let (future, sleep) = unsafe {
            let this = self.get_unchecked_mut();
            (
                Pin::new_unchecked(&mut this.future),
                Pin::new_unchecked(&mut this.sleep),
            )
        };

        if let Poll::Ready(val) = future.poll(cx) {
            return Poll::Ready(Ok(val));
        }

        if let Poll::Ready(()) = sleep.poll(cx) {
            return Poll::Ready(Err(Elapsed::new()));
        }

        Poll::Pending
    }
}
