/*  This file is part of Loyalty Card App.
 *
 *  Copyright (C) 2026 Marko Ivankovic
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU Affero General Public License as published
 *  by the Free Software Foundation, version 3 of the License.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 *  GNU Affero General Public License for more details.
 *
 *  You should have received a copy of the GNU Affero General Public License
 *  along with this program. If not, see <https://www.gnu.org/licenses/>.
 */

//! The accept loop.
//!
//! A loop of its own rather than `axum::serve`, which gives hyper no timer, so no header read
//! timeout, and accepts without limit: a few hundred half-sent requests held open would each keep
//! a buffer and a descriptor. Here a request head has [`ServeLimits::header_read_timeout`] to
//! arrive and [`ServeLimits::max_header_bytes`] to fit in, and at most
//! [`ServeLimits::max_connections`] are open at once; past that, connections wait in the
//! kernel's backlog, not in this process.
//!
//! The peer address goes into each request as axum's `ConnectInfo`, for the request log and the
//! sign-in throttle.

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use std::net::SocketAddr;

#[derive(Debug, Clone, Copy)]
pub struct ServeLimits {
    /// From the moment the server is ready for a request line to the end of the headers. Also
    /// how long an idle keep-alive connection lasts.
    pub header_read_timeout: std::time::Duration,
    /// Connections open at once.
    pub max_connections: usize,
    /// The most a connection may buffer of an unfinished request head.
    pub max_header_bytes: usize,
}

impl Default for ServeLimits {
    fn default() -> Self {
        Self {
            header_read_timeout: std::time::Duration::from_secs(10),
            // A household's phones, a few connections each, many times over.
            max_connections: 256,
            // A real request head here is a few hundred bytes and one cookie.
            max_header_bytes: 16 * 1024,
        }
    }
}

/// Binds `addr` and serves `app` until the process is told to stop (SIGTERM or Ctrl-C).
/// Prints the address it listens on, which `--listen 127.0.0.1:0` makes useful.
pub async fn serve(app: Router, addr: SocketAddr, limits: ServeLimits) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| anyhow::anyhow!("cannot listen on {addr}: {e}"))?;
    eprintln!("listening on {}", listener.local_addr()?);
    tokio::select! {
        served = serve_on(listener, app, limits) => served,
        () = stop_signal() => {
            eprintln!("stopping");
            Ok(())
        }
    }
}

async fn stop_signal() {
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending().await,
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        () = terminate => {}
    }
}

/// Serves `app` on `listener`, with [`ServeLimits`], until accepting fails for good.
pub async fn serve_on(
    listener: tokio::net::TcpListener,
    app: Router,
    limits: ServeLimits,
) -> anyhow::Result<()> {
    use hyper_util::rt::{TokioIo, TokioTimer};
    use tower::ServiceExt as _;

    let slots = std::sync::Arc::new(tokio::sync::Semaphore::new(limits.max_connections));
    loop {
        // A slot first, then the accept: past the cap, connections wait in the kernel.
        let slot = slots
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| anyhow::anyhow!("the web server stopped: {e}"))?;
        let (stream, peer) = match listener.accept().await {
            Ok(accepted) => accepted,
            Err(err) => {
                // Out of descriptors, or a connection reset before it was accepted: try again
                // shortly rather than spin or stop serving.
                eprintln!("accept failed: {err}");
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }
        };
        let app = app.clone();
        tokio::spawn(async move {
            let _slot = slot;
            let service =
                hyper::service::service_fn(move |request: Request<hyper::body::Incoming>| {
                    let mut request = request.map(Body::new);
                    request
                        .extensions_mut()
                        .insert(axum::extract::ConnectInfo(peer));
                    app.clone().oneshot(request)
                });
            let mut http = hyper::server::conn::http1::Builder::new();
            http.timer(TokioTimer::new())
                .header_read_timeout(limits.header_read_timeout)
                .max_buf_size(limits.max_header_bytes.max(8192));
            // An error here is a client that went away or timed out: nothing to do about it.
            let _ = http.serve_connection(TokioIo::new(stream), service).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    async fn started(limits: ServeLimits) -> SocketAddr {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new().route("/", axum::routing::get(|| async { "hello" }));
        tokio::spawn(serve_on(listener, app, limits));
        addr
    }

    #[tokio::test]
    async fn a_request_is_answered() {
        let addr = started(ServeLimits::default()).await;
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).await.unwrap();
        assert!(
            answer.starts_with("HTTP/1.1 200"),
            "the answer was:\n{answer}"
        );
        assert!(answer.ends_with("hello"), "the answer was:\n{answer}");
    }

    #[tokio::test]
    async fn a_head_that_never_ends_is_hung_up_on() {
        let addr = started(ServeLimits {
            header_read_timeout: std::time::Duration::from_millis(100),
            ..ServeLimits::default()
        })
        .await;
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: x\r\n")
            .await
            .unwrap();
        let mut answer = Vec::new();
        let read = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            stream.read_to_end(&mut answer),
        )
        .await;
        assert!(
            read.is_ok(),
            "the connection was still open after two seconds"
        );
    }

    #[tokio::test]
    async fn past_the_cap_a_connection_waits_for_a_slot() {
        let addr = started(ServeLimits {
            max_connections: 1,
            ..ServeLimits::default()
        })
        .await;
        let mut holder = tokio::net::TcpStream::connect(addr).await.unwrap();
        holder
            .write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n")
            .await
            .unwrap();
        let mut first = [0u8; 12];
        holder.read_exact(&mut first).await.unwrap();
        let mut waiting = tokio::net::TcpStream::connect(addr).await.unwrap();
        waiting
            .write_all(b"GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut answer = [0u8; 12];
        let early = tokio::time::timeout(
            std::time::Duration::from_millis(150),
            waiting.read_exact(&mut answer),
        )
        .await;
        assert!(
            early.is_err(),
            "a second connection was served past the cap of one"
        );
        drop(holder);
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            waiting.read_exact(&mut answer),
        )
        .await
        .expect("the waiting connection was never served")
        .unwrap();
        assert_eq!(&answer, b"HTTP/1.1 200");
    }
}
