// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

use crate::threading::sync::oneshot;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

pub struct JoinHandle<T> {
    pub(crate) rx: oneshot::Receiver<Result<T, JoinError>>,
    pub(crate) aborted: Arc<AtomicBool>,
}

impl<T> JoinHandle<T> {
    pub fn abort(&self) {
        self.aborted.store(true, Ordering::SeqCst);
    }

    pub fn is_finished(&self) -> bool {
        self.aborted.load(Ordering::SeqCst)
    }
}

impl<T> Future for JoinHandle<T> {
    type Output = Result<T, JoinError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.rx).poll(cx) {
            Poll::Ready(Ok(res)) => Poll::Ready(res),
            Poll::Ready(Err(_)) => Poll::Ready(Err(JoinError::cancelled())),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[derive(Debug)]
pub struct JoinError {
    is_cancelled: bool,
    is_panic: bool,
}

impl JoinError {
    pub fn cancelled() -> Self {
        JoinError {
            is_cancelled: true,
            is_panic: false,
        }
    }

    pub fn panic() -> Self {
        JoinError {
            is_cancelled: false,
            is_panic: true,
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.is_cancelled
    }

    pub fn is_panic(&self) -> bool {
        self.is_panic
    }
}

impl fmt::Display for JoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_cancelled {
            write!(f, "task was cancelled")
        } else {
            write!(f, "task panicked")
        }
    }
}

impl std::error::Error for JoinError {}

pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    crate::threading::runtime::Handle::current().spawn(future)
}

pub fn spawn_blocking<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    crate::threading::runtime::Handle::current().spawn_blocking(f)
}
