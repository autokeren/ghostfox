//! Juggler pipe transport.
//!
//! The browser's juggler channel lives on **fd 3 (commands, we→browser)** and
//! **fd 4 (responses, browser→we)** per the Playwright FirefoxConnection
//! convention (`stdio = [ignore, pipe, pipe, pipe, pipe]`). stdio 0/1/2 stay
//! free for the engine's own logs.
//!
//! Message shape: `{"id": n, "method": "Domain.method", "params": {...}}` →
//! `{"id": n, "result": {...}}` or `{"id": n, "error": {...}}`. Events arrive
//! as `{"method": ..., "params": ...}` with no id.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use ghostcloak_core::error::{GhostError, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Child;
use tokio::sync::{Mutex, oneshot};

pub struct JugglerConnection {
    child: Mutex<std::process::Child>,
    /// fd 3: commands we write to the browser.
    stdin: Mutex<tokio::io::BufWriter<tokio::fs::File>>,
    next_id: AtomicU64,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<serde_json::Value>>>>,
    /// Broadcast of every event message (no id) for listeners that need
    /// frame/context discovery.
    events: tokio::sync::broadcast::Sender<serde_json::Value>,
    /// Fires once when the pipe dies; all in-flight requests get an error.
    closed: Arc<tokio::sync::Notify>,
    /// Latched mirror of `closed` for synchronous checks.
    dead: std::sync::atomic::AtomicBool,
    _pid: u32,
}

impl JugglerConnection {
    /// Take ownership of the browser child (std) and our side of the juggler
    /// pipes (fd 3 writer, fd 4 reader). A reaper task waits on the child so
    /// it never zombies and `closed` fires when the browser exits.
    pub fn spawn_std(
        mut child: std::process::Child,
        pid: u32,
        cmd_tx: os_pipe::PipeWriter,
        resp_rx: os_pipe::PipeReader,
    ) -> Result<Arc<Self>> {
        // Convert the std Child into a tokio-managed one via kill_on_drop:
        // tokio has no From<std::process::Child>, so we own it in a blocking
        // task and report exit through `closed`.
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                use std::io::Read;
                let mut stderr = stderr;
                let mut buf = [0u8; 4096];
                while stderr.read(&mut buf).is_ok() {}
            });
        }

        // Our ends of the juggler pipes, converted to tokio async IO via
        // OwnedFd (os_pipe types convert through their raw fds).
        let stdin = tokio::io::BufWriter::new(tokio::fs::File::from(
            std::os::fd::OwnedFd::from(cmd_tx),
        ));
        let stdout = tokio::fs::File::from(std::os::fd::OwnedFd::from(resp_rx));

        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<serde_json::Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (events, _) = tokio::sync::broadcast::channel::<serde_json::Value>(256);
        let events_tx = events.clone();

        let closed = Arc::new(tokio::sync::Notify::new());
        let conn = Arc::new(Self {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            next_id: AtomicU64::new(1),
            pending: pending.clone(),
            events,
            closed: closed.clone(),
            dead: std::sync::atomic::AtomicBool::new(false),
            _pid: pid,
        });

        // Wait for the child to exit in the background; fire `closed` then.
        let dead = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let dead_reader = dead.clone();
        tokio::spawn(reader_task(stdout, pending, events_tx, closed.clone(), dead_reader));
        // Mirror the reader's latch into the connection (weak updater).
        let conn2 = conn.clone();
        tokio::spawn(async move {
            loop {
                if dead.load(std::sync::atomic::Ordering::SeqCst) {
                    conn2.dead.store(true, std::sync::atomic::Ordering::SeqCst);
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        });

        Ok(conn)
    }

    /// Send a command and await its response.
    ///
    /// Juggler framing (from Playwright's PipeTransport): each message is
    /// the JSON bytes followed by a single `\0` terminator — no newlines.
    pub async fn request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.request_session(method, params, None).await
    }

    /// Same as [`request`], but routed to a target's session. Page/Runtime/
    /// Network commands only exist inside a session; Browser commands use
    /// the root session (no id).
    pub async fn request_session(
        &self,
        method: &str,
        params: serde_json::Value,
        session_id: Option<&str>,
    ) -> Result<serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut msg = serde_json::json!({
            "id": id,
            "method": method,
            "params": params,
        });
        if let Some(sid) = session_id {
            msg["sessionId"] = serde_json::Value::String(sid.to_string());
        }
        let mut line = serde_json::to_string(&msg)
            .map_err(|e| GhostError::Protocol(e.to_string()))?;
        line.push('\0');

        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        {
            let mut stdin = self.stdin.lock().await;
            stdin
                .write_all(line.as_bytes())
                .await
                .map_err(|e| GhostError::Protocol(format!("juggler write: {e}")))?;
            stdin
                .flush()
                .await
                .map_err(|e| GhostError::Protocol(format!("juggler flush: {e}")))?;
        }

        let resp = tokio::select! {
            r = rx => r.map_err(|_| GhostError::EngineCrashed)?,
            _ = self.closed.notified() => return Err(GhostError::EngineCrashed),
            // Requests on a wedged page (heavy workers, redirect storms)
            // must not pin the caller forever. Late responses land in the
            // pending map and are dropped.
            _ = tokio::time::sleep(std::time::Duration::from_secs(20)) => {
                self.pending.lock().await.remove(&id);
                return Err(GhostError::Protocol(format!("juggler: timeout waiting for {method}")));
            }
        };

        if let Some(err) = resp.get("error") {
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown juggler error");
            return Err(GhostError::PageOp(msg.to_string()));
        }
        Ok(resp.get("result").cloned().unwrap_or(serde_json::Value::Null))
    }

    /// Subscribe to event messages (frames attaching, contexts created...).
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<serde_json::Value> {
        self.events.subscribe()
    }

    /// True once the pipe has died (browser exited or crashed).
    pub fn is_closed(&self) -> bool {
        self.dead.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Synchronous last-resort kill (Drop path): SIGKILL the process group.
    pub fn kill_now(&self) {
        if let Ok(child) = self.child.try_lock() {
            let pid = child.id() as i32;
            unsafe {
                // Negative pid = process group; child was setsid'd.
                libc::kill(pid, libc::SIGKILL);
            }
        }
        self.dead.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub async fn kill(&self) {
        if let Ok(mut child) = self.child.try_lock() {
            let pid = child.id() as i32;
            // The child was spawned with setsid(), so pid == pgid: killing
            // the negative pid signals the whole tree (contentprocs die too).
            let _ = tokio::task::spawn_blocking(move || {
                unsafe {
                    libc::kill(pid, libc::SIGTERM);
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                    loop {
                        let gone = unsafe { libc::kill(pid, 0) != 0 };
                        if gone || std::time::Instant::now() > deadline {
                            break gone;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                }
            })
            .await;
            let _ = child.kill();
            let _ = child.wait();
        }
        self.dead.store(true, std::sync::atomic::Ordering::SeqCst);
        self.closed.notify_waiters();
    }
}

async fn reader_task(
    stdout: tokio::fs::File,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<serde_json::Value>>>>,
    events: tokio::sync::broadcast::Sender<serde_json::Value>,
    closed: Arc<tokio::sync::Notify>,
    dead: Arc<std::sync::atomic::AtomicBool>,
) {
    let mut reader = BufReader::new(stdout);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        // Messages are `\0`-terminated (Playwright PipeTransport framing).
        match reader.read_until(b'\0', &mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                // Strip the terminator.
                if buf.last() == Some(&b'\0') {
                    buf.pop();
                }
                if buf.is_empty() {
                    continue;
                }
                let Ok(msg) = serde_json::from_slice::<serde_json::Value>(&buf) else {
                    tracing::warn!(target: "ghostcloak::juggler", "unparseable: {}", String::from_utf8_lossy(&buf));
                    continue;
                };
                if let Some(id) = msg.get("id").and_then(|i| i.as_u64()) {
                    tracing::trace!(target: "ghostcloak::juggler", "response {id}: {}", String::from_utf8_lossy(&buf).chars().take(300).collect::<String>());
                    if let Some(tx) = pending.lock().await.remove(&id) {
                        let _ = tx.send(msg);
                    }
                } else if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
                    tracing::trace!(target: "ghostcloak::juggler", "event: {method}");
                    // Fan out to subscribers (frame/context discovery).
                    let _ = events.send(msg);
                }
            }
        }
    }
    // Signal both the waiters and the latched flag.
    dead.store(true, std::sync::atomic::Ordering::SeqCst);
    closed.notify_waiters();
}
