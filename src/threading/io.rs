// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

use std::future::Future;
use std::io;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct ReadBuf<'a> {
    buf: &'a mut [MaybeUninit<u8>],
    filled: usize,
    initialized: usize,
}

impl<'a> ReadBuf<'a> {
    #[inline]
    pub fn new(buf: &'a mut [u8]) -> ReadBuf<'a> {
        let len = buf.len();
        let buf = unsafe { &mut *(buf as *mut [u8] as *mut [MaybeUninit<u8>]) };
        ReadBuf {
            buf,
            filled: 0,
            initialized: len,
        }
    }

    #[inline]
    pub fn filled(&self) -> &[u8] {
        let slice = &self.buf[..self.filled];
        unsafe { &*(slice as *const [MaybeUninit<u8>] as *const [u8]) }
    }

    #[inline]
    pub fn filled_mut(&mut self) -> &mut [u8] {
        let slice = &mut self.buf[..self.filled];
        unsafe { &mut *(slice as *mut [MaybeUninit<u8>] as *mut [u8]) }
    }

    #[inline]
    pub fn initialize_unfilled(&mut self) -> &mut [u8] {
        self.initialize_unfilled_to(self.remaining())
    }

    #[inline]
    pub fn initialize_unfilled_to(&mut self, n: usize) -> &mut [u8] {
        let end = (self.filled + n).min(self.buf.len());
        if end > self.initialized {
            for slot in &mut self.buf[self.initialized..end] {
                slot.write(0);
            }
            self.initialized = end;
        }
        let slice = &mut self.buf[self.filled..end];
        unsafe { &mut *(slice as *mut [MaybeUninit<u8>] as *mut [u8]) }
    }

    #[inline]
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.filled
    }

    #[inline]
    pub fn advance(&mut self, n: usize) {
        let new_filled = self.filled + n;
        assert!(new_filled <= self.buf.len());
        self.filled = new_filled;
        if self.filled > self.initialized {
            self.initialized = self.filled;
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.filled = 0;
    }
}

pub trait AsyncRead {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>>;
}

pub trait AsyncWrite {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>>;

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>>;

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>>;
}

pub trait AsyncReadExt: AsyncRead {
    fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> Read<'a, Self>
    where
        Self: Unpin,
    {
        Read { reader: self, buf }
    }

    fn read_exact<'a>(&'a mut self, buf: &'a mut [u8]) -> ReadExact<'a, Self>
    where
        Self: Unpin,
    {
        ReadExact { reader: self, buf, pos: 0 }
    }

    fn read_to_end<'a>(&'a mut self, buf: &'a mut Vec<u8>) -> ReadToEnd<'a, Self>
    where
        Self: Unpin,
    {
        ReadToEnd { reader: self, buf }
    }
}

impl<R: AsyncRead + ?Sized> AsyncReadExt for R {}

pub struct Read<'a, R: ?Sized> {
    reader: &'a mut R,
    buf: &'a mut [u8],
}

impl<'a, R: AsyncRead + Unpin + ?Sized> Future for Read<'a, R> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        let mut read_buf = ReadBuf::new(this.buf);
        match Pin::new(&mut *this.reader).poll_read(cx, &mut read_buf) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(read_buf.filled().len())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub struct ReadExact<'a, R: ?Sized> {
    reader: &'a mut R,
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a, R: AsyncRead + Unpin + ?Sized> Future for ReadExact<'a, R> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        while this.pos < this.buf.len() {
            let mut read_buf = ReadBuf::new(&mut this.buf[this.pos..]);
            match Pin::new(&mut *this.reader).poll_read(cx, &mut read_buf) {
                Poll::Ready(Ok(())) => {
                    let n = read_buf.filled().len();
                    if n == 0 {
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "early eof",
                        )));
                    }
                    this.pos += n;
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }
        Poll::Ready(Ok(this.pos))
    }
}

pub struct ReadToEnd<'a, R: ?Sized> {
    reader: &'a mut R,
    buf: &'a mut Vec<u8>,
}

impl<'a, R: AsyncRead + Unpin + ?Sized> Future for ReadToEnd<'a, R> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        let start_len = this.buf.len();
        let mut chunk = [0u8; 8192];
        loop {
            let mut read_buf = ReadBuf::new(&mut chunk);
            match Pin::new(&mut *this.reader).poll_read(cx, &mut read_buf) {
                Poll::Ready(Ok(())) => {
                    let filled = read_buf.filled();
                    if filled.is_empty() {
                        return Poll::Ready(Ok(this.buf.len() - start_len));
                    }
                    this.buf.extend_from_slice(filled);
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => {
                    return Poll::Pending;
                }
            }
        }
    }
}

pub trait AsyncWriteExt: AsyncWrite {
    fn write<'a>(&'a mut self, buf: &'a [u8]) -> WriteFuture<'a, Self>
    where
        Self: Unpin,
    {
        WriteFuture { writer: self, buf }
    }

    fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> WriteAllFuture<'a, Self>
    where
        Self: Unpin,
    {
        WriteAllFuture { writer: self, buf }
    }

    fn flush(&mut self) -> FlushFuture<'_, Self>
    where
        Self: Unpin,
    {
        FlushFuture { writer: self }
    }
}

impl<W: AsyncWrite + ?Sized> AsyncWriteExt for W {}

pub struct WriteFuture<'a, W: ?Sized> {
    writer: &'a mut W,
    buf: &'a [u8],
}

impl<'a, W: AsyncWrite + Unpin + ?Sized> Future for WriteFuture<'a, W> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        Pin::new(&mut *this.writer).poll_write(cx, this.buf)
    }
}

pub struct WriteAllFuture<'a, W: ?Sized> {
    writer: &'a mut W,
    buf: &'a [u8],
}

impl<'a, W: AsyncWrite + Unpin + ?Sized> Future for WriteAllFuture<'a, W> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        while !this.buf.is_empty() {
            match Pin::new(&mut *this.writer).poll_write(cx, this.buf) {
                Poll::Ready(Ok(0)) => {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "failed to write whole buffer",
                    )));
                }
                Poll::Ready(Ok(n)) => {
                    this.buf = &this.buf[n..];
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }
        Poll::Ready(Ok(()))
    }
}

pub struct FlushFuture<'a, W: ?Sized> {
    writer: &'a mut W,
}

impl<'a, W: AsyncWrite + Unpin + ?Sized> Future for FlushFuture<'a, W> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        Pin::new(&mut *this.writer).poll_flush(cx)
    }
}
