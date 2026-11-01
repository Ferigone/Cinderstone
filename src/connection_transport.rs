//! Connection-wide raw ingress and the original TCP read-inactivity timeout.
//!
//! Java installs `ReadTimeoutHandler(30)` before decryption and framing. A byte
//! arriving in an incomplete frame therefore refreshes inactivity, while a
//! decoded packet, a write, or replacing the active listener does not. Keep the
//! pump and its watchdog for the entire actual TCP connection.

use std::fmt;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, ReadBuf};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::{self, Instant};

/// `ServerConnectionListener$1.initChannel` uses seconds, not server ticks.
pub const READ_TIMEOUT: Duration = Duration::from_secs(30);
pub const READ_TIMEOUT_REASON: &str = "disconnect.timeout";

/// A native memory bound, not a vanilla packet-size or player-rate rule.
pub const RAW_READ_CHUNK_BYTES: usize = 8_192;
pub const RAW_READ_QUEUED_CHUNKS: usize = 8;

/// A Netty read timeout closes the actual channel. It does not first send a
/// LOGIN, CONFIGURATION or PLAY disconnect packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadTimeout;

impl ReadTimeout {
    pub const fn reason(self) -> &'static str {
        READ_TIMEOUT_REASON
    }
}

impl fmt::Display for ReadTimeout {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(READ_TIMEOUT_REASON)
    }
}

impl std::error::Error for ReadTimeout {}

/// The outer connection driver must poll this concurrently with its complete
/// session future, including authentication and other listener work. Wrapping
/// only an awaited packet decoder would give packet-completion timeouts and
/// stop the clock while that decoder was not polled.
#[derive(Clone, Debug)]
pub struct ReadTimeoutWatch {
    raw_activity: watch::Receiver<Instant>,
}

impl ReadTimeoutWatch {
    /// Wait for thirty seconds without a successful nonempty raw socket read.
    ///
    /// A coalesced watch update retains the latest observed raw-read time. The
    /// latest timestamp is rechecked after the timer fires so a concurrently
    /// completed socket read does not inherit the previous deadline. Once the
    /// pump has ended, this future stays pending: buffered EOF or its original
    /// I/O error then belongs to `RawReadStream`, not a later invented timeout.
    pub async fn wait(&mut self) -> ReadTimeout {
        loop {
            let deadline = *self.raw_activity.borrow_and_update() + READ_TIMEOUT;
            tokio::select! {
                biased;
                changed = self.raw_activity.changed() => {
                    if changed.is_err() {
                        return std::future::pending().await;
                    }
                }
                _ = time::sleep_until(deadline) => {
                    let last_read = *self.raw_activity.borrow_and_update();
                    if Instant::now().saturating_duration_since(last_read) >= READ_TIMEOUT {
                        return ReadTimeout;
                    }
                }
            }
        }
    }
}

/// Holds raw bytes only. The caller's existing `CryptoRead` must wrap this
/// stream so an encryption switch cannot reinterpret already decrypted bytes.
pub struct RawReadStream {
    chunks: mpsc::Receiver<io::Result<Vec<u8>>>,
    current: Vec<u8>,
    offset: usize,
    finished: bool,
}

impl AsyncRead for RawReadStream {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        output: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if output.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        loop {
            if this.offset < this.current.len() {
                let count = output.remaining().min(this.current.len() - this.offset);
                output.put_slice(&this.current[this.offset..this.offset + count]);
                this.offset += count;
                return Poll::Ready(Ok(()));
            }
            if this.finished {
                return Poll::Ready(Ok(()));
            }
            match this.chunks.poll_recv(context) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(Ok(chunk))) => {
                    this.current = chunk;
                    this.offset = 0;
                }
                Poll::Ready(Some(Err(error))) => {
                    this.finished = true;
                    return Poll::Ready(Err(error));
                }
                Poll::Ready(None) => {
                    this.finished = true;
                    return Poll::Ready(Ok(()));
                }
            }
        }
    }
}

/// Dropping the actual connection owner stops its raw-read task. Merely
/// dropping a Tokio JoinHandle would leave a detached socket reader running.
pub struct RawReadPumpGuard(JoinHandle<()>);

impl RawReadPumpGuard {
    /// Lets the actual connection's terminal/read-only owner stop further raw
    /// ingress while retaining its writer until an already-dispatched terminal
    /// packet completes. This does not dispatch the bytes already in the queue.
    pub fn abort_handle(&self) -> tokio::task::AbortHandle {
        self.0.abort_handle()
    }
}

impl Drop for RawReadPumpGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Start raw ingress at TCP activation, before reading the handshake.
///
/// The task reads independently of packet processing. It never decrypts,
/// decompresses, decodes, discards, or replaces a protocol/listener. Partial
/// frame/header bytes survive LOGIN encryption and all later phase changes.
/// Original I/O errors follow already-read bytes in the same bounded stream.
///
/// Backpressure is a declared native resource boundary: eight queued 8-KiB
/// chunks, the consumer's current chunk and at most one producer chunk. When
/// that queue is full, further socket reads wait for the actual consumer; this
/// is not a claim of Netty event-loop/backpressure equivalence under saturation.
pub fn spawn_raw_read_transport<R>(
    mut socket_reader: R,
) -> (RawReadPumpGuard, RawReadStream, ReadTimeoutWatch)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    let (sender, chunks) = mpsc::channel(RAW_READ_QUEUED_CHUNKS);
    let (activity, raw_activity) = watch::channel(Instant::now());
    let pump = tokio::spawn(async move {
        loop {
            let mut input = vec![0; RAW_READ_CHUNK_BYTES];
            match socket_reader.read(&mut input).await {
                Ok(0) => break,
                Ok(count) => {
                    // Observe real ingress, before the bounded downstream queue
                    // and before any decryption, framing or packet dispatch.
                    activity.send_replace(Instant::now());
                    input.truncate(count);
                    if sender.send(Ok(input)).await.is_err() {
                        break;
                    }
                }
                Err(error) => {
                    // Stop inactivity ownership immediately, even if the
                    // ordered error waits behind a full downstream queue.
                    drop(activity);
                    let _ = sender.send(Err(error)).await;
                    break;
                }
            }
        }
    });
    (
        RawReadPumpGuard(pump),
        RawReadStream {
            chunks,
            current: Vec::new(),
            offset: 0,
            finished: false,
        },
        ReadTimeoutWatch { raw_activity },
    )
}
