// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

use crate::threading::io::{AsyncRead, AsyncWrite, ReadBuf};
use std::collections::HashMap;
use std::future::Future;
use std::io::{self, ErrorKind, Read, Write};
use std::net::{SocketAddr, ToSocketAddrs as StdToSocketAddrs};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll, Waker};
use std::thread;

struct IoPoller {
    wakers: HashMap<u64, Waker>,
}

static IO_POLLER: OnceLock<Arc<Mutex<IoPoller>>> = OnceLock::new();
static NEXT_IO_ID: AtomicU64 = AtomicU64::new(1);

fn get_io_poller() -> &'static Arc<Mutex<IoPoller>> {
    IO_POLLER.get_or_init(|| {
        let poller = Arc::new(Mutex::new(IoPoller {
            wakers: HashMap::new(),
        }));

        let poller_clone = Arc::clone(&poller);
        thread::Builder::new()
            .name("arkcode-net-poller".into())
            .spawn(move || loop {
                let wakers: Vec<Waker> = {
                    let guard = poller_clone.lock().unwrap();
                    guard.wakers.values().cloned().collect()
                };

                let has_wakers = !wakers.is_empty();
                for waker in wakers {
                    waker.wake();
                }

                if has_wakers {
                    thread::sleep(std::time::Duration::from_millis(2));
                } else {
                    thread::sleep(std::time::Duration::from_millis(15));
                }
            })
            .expect("Failed to spawn IO poller thread");

        poller
    })
}

pub(crate) fn register_io_waker(id: u64, waker: &Waker) {
    let poller = get_io_poller();
    let mut guard = poller.lock().unwrap();
    guard.wakers.insert(id, waker.clone());
}

pub(crate) fn unregister_io_waker(id: u64) {
    let poller = get_io_poller();
    let mut guard = poller.lock().unwrap();
    guard.wakers.remove(&id);
}

pub trait ToSocketAddrs: Send + Sync {
    fn to_socket_addrs(&self) -> io::Result<Vec<SocketAddr>>;
}

impl<T: StdToSocketAddrs + Send + Sync + ?Sized> ToSocketAddrs for T {
    fn to_socket_addrs(&self) -> io::Result<Vec<SocketAddr>> {
        StdToSocketAddrs::to_socket_addrs(self).map(|iter| iter.collect())
    }
}

#[derive(Debug)]
pub struct TcpListener {
    inner: Arc<std::net::TcpListener>,
    id: u64,
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        unregister_io_waker(self.id);
    }
}

impl TcpListener {
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<TcpListener> {
        let addrs = addr.to_socket_addrs()?;
        let mut last_err = None;

        for a in addrs {
            match std::net::TcpListener::bind(a) {
                Ok(listener) => {
                    listener.set_nonblocking(true)?;
                    return Ok(TcpListener {
                        inner: Arc::new(listener),
                        id: NEXT_IO_ID.fetch_add(1, Ordering::Relaxed),
                    });
                }
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "could not resolve to any addresses")
        }))
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    pub fn poll_accept(&self, cx: &mut Context<'_>) -> Poll<io::Result<(TcpStream, SocketAddr)>> {
        match self.inner.accept() {
            Ok((stream, addr)) => {
                stream.set_nonblocking(true)?;
                let tcp_stream = TcpStream {
                    inner: Arc::new(stream),
                    id: NEXT_IO_ID.fetch_add(1, Ordering::Relaxed),
                };
                Poll::Ready(Ok((tcp_stream, addr)))
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                register_io_waker(self.id, cx.waker());
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    pub async fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        struct AcceptFuture<'a>(&'a TcpListener);
        impl<'a> Future for AcceptFuture<'a> {
            type Output = io::Result<(TcpStream, SocketAddr)>;
            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                self.0.poll_accept(cx)
            }
        }
        AcceptFuture(self).await
    }
}

#[derive(Debug)]
pub struct TcpStream {
    inner: Arc<std::net::TcpStream>,
    id: u64,
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        unregister_io_waker(self.id);
    }
}

impl TcpStream {
    pub async fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<TcpStream> {
        let addrs = addr.to_socket_addrs()?;
        let mut last_err = None;

        for a in addrs {
            match std::net::TcpStream::connect_timeout(&a, std::time::Duration::from_secs(5)) {
                Ok(stream) => {
                    stream.set_nonblocking(true)?;
                    return Ok(TcpStream {
                        inner: Arc::new(stream),
                        id: NEXT_IO_ID.fetch_add(1, Ordering::Relaxed),
                    });
                }
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "could not resolve to any addresses")
        }))
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let unfilled = buf.initialize_unfilled();
        match (&*self.inner).read(unfilled) {
            Ok(n) => {
                buf.advance(n);
                Poll::Ready(Ok(()))
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                register_io_waker(self.id, cx.waker());
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

impl AsyncWrite for TcpStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match (&*self.inner).write(buf) {
            Ok(n) => Poll::Ready(Ok(n)),
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                register_io_waker(self.id, cx.waker());
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match (&*self.inner).flush() {
            Ok(()) => Poll::Ready(Ok(())),
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.inner.shutdown(std::net::Shutdown::Write) {
            Ok(()) => Poll::Ready(Ok(())),
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}
