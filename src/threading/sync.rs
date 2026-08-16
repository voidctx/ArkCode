// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex as StdMutex};
use std::task::{Context, Poll, Waker};

pub mod oneshot {
    use super::*;

    pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
        let inner = Arc::new(StdMutex::new(Inner {
            value: None,
            waker: None,
            is_closed: false,
        }));
        (
            Sender { inner: Arc::clone(&inner) },
            Receiver { inner },
        )
    }

    struct Inner<T> {
        value: Option<T>,
        waker: Option<Waker>,
        is_closed: bool,
    }

    pub struct Sender<T> {
        inner: Arc<StdMutex<Inner<T>>>,
    }

    impl<T> Sender<T> {
        pub fn send(self, t: T) -> Result<(), T> {
            let mut guard = self.inner.lock().unwrap();
            if guard.is_closed {
                return Err(t);
            }
            guard.value = Some(t);
            if let Some(waker) = guard.waker.take() {
                waker.wake();
            }
            Ok(())
        }
    }

    impl<T> Drop for Sender<T> {
        fn drop(&mut self) {
            let mut guard = self.inner.lock().unwrap();
            guard.is_closed = true;
            if let Some(waker) = guard.waker.take() {
                waker.wake();
            }
        }
    }

    pub struct Receiver<T> {
        inner: Arc<StdMutex<Inner<T>>>,
    }

    impl<T> Future for Receiver<T> {
        type Output = Result<T, RecvError>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let mut guard = self.inner.lock().unwrap();
            if let Some(val) = guard.value.take() {
                return Poll::Ready(Ok(val));
            }
            if guard.is_closed {
                return Poll::Ready(Err(RecvError));
            }
            guard.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct RecvError;

    impl fmt::Display for RecvError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "channel closed")
        }
    }

    impl std::error::Error for RecvError {}
}
