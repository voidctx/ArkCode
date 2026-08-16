// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

pub mod io;
pub mod net;
pub mod runtime;
pub mod sync;
pub mod task;
pub mod time;

pub use task::{spawn, spawn_blocking};
pub use time::{sleep, timeout, Duration, Instant};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::threading::io::{AsyncReadExt, AsyncWriteExt};
    use crate::threading::net::{TcpListener, TcpStream};
    use crate::threading::sync::oneshot;

    #[test]
    fn test_runtime_spawn_and_timers() {
        let rt = runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let start = Instant::now();
            sleep(Duration::from_millis(30)).await;
            assert!(start.elapsed() >= Duration::from_millis(20));

            let handle = spawn(async {
                sleep(Duration::from_millis(10)).await;
                42
            });

            assert_eq!(handle.await.unwrap(), 42);

            let res = timeout(Duration::from_millis(100), async {
                sleep(Duration::from_millis(10)).await;
                "ok"
            })
            .await;
            assert_eq!(res.unwrap(), "ok");

            let timed_out = timeout(Duration::from_millis(10), async {
                sleep(Duration::from_millis(100)).await;
                "fail"
            })
            .await;
            assert!(timed_out.is_err());
        });
    }

    #[test]
    fn test_runtime_tcp_echo() {
        let rt = runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();

            let server = spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buf = [0u8; 16];
                let n = stream.read(&mut buf).await.unwrap();
                stream.write_all(&buf[..n]).await.unwrap();
            });

            let client = spawn(async move {
                let mut stream = TcpStream::connect(addr).await.unwrap();
                stream.write_all(b"ping echo").await.unwrap();
                let mut resp = vec![0u8; 9];
                stream.read_exact(&mut resp).await.unwrap();
                assert_eq!(&resp[..], b"ping echo");
            });

            server.await.unwrap();
            client.await.unwrap();
        });
    }

    #[test]
    fn test_oneshot_channel() {
        let rt = runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let (tx, rx) = oneshot::channel();
            spawn(async move {
                tx.send("hello").unwrap();
            });
            assert_eq!(rx.await.unwrap(), "hello");
        });
    }
}
