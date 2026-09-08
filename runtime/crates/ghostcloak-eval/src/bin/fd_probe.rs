//! Minimal fd3/fd4 inheritance test: spawn camoufox-bin with dup2 in
//! pre_exec, then poll our ends for EOF/timeout. Isolates whether the
//! juggler pipes survive the Rust std spawn path.

use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;

fn main() {
    // os_pipe::pipe() -> (reader, writer).
    let (cmd_rx, cmd_tx) = os_pipe::pipe().unwrap();
    let (resp_rx, resp_tx) = os_pipe::pipe().unwrap();

    let cmd_rx_fd = cmd_rx.as_raw_fd();
    let resp_tx_fd = resp_tx.as_raw_fd();
    // Keep alive for the child's lifetime.
    std::mem::forget(cmd_rx);
    std::mem::forget(resp_tx);

    let mut cmd = std::process::Command::new("/data/user_cache/camoufox/camoufox-bin");
    cmd.args(["--headless", "--juggler-pipe", "-silent", "-profile", "/tmp/fdp-p", "-no-remote"])
        .current_dir("/data/user_cache/camoufox")
        .env("CAMOU_CONFIG_1", "{}")
        .env("HOME", "/home/ubuntu")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    unsafe {
        cmd.pre_exec(move || {
            libc::dup2(cmd_rx_fd, 3);
            libc::dup2(resp_tx_fd, 4);
            let f3 = libc::fcntl(3, libc::F_GETFD);
            libc::fcntl(3, libc::F_SETFD, f3 & !libc::FD_CLOEXEC);
            let f4 = libc::fcntl(4, libc::F_GETFD);
            libc::fcntl(4, libc::F_SETFD, f4 & !libc::FD_CLOEXEC);
            Ok(())
        });
    }

    std::fs::create_dir_all("/tmp/fdp-p").unwrap();
    let mut child = cmd.spawn().unwrap();
    println!("spawned pid {}", child.id());

    // Poll resp_rx for EOF or send a command.
    std::thread::sleep(std::time::Duration::from_secs(5));

    use std::io::Write;
    let mut w = cmd_tx;
    let msg = br#"{"id":1,"method":"Browser.enable","params":{"attachToDefaultContext":true}}"#;
    match w.write_all(msg) {
        Ok(_) => println!("write ok"),
        Err(e) => println!("WRITE FAILED: {e}"),
    }
    let _ = w.write_all(b"\0");
    let _ = w.flush();

    // Read response with 8s timeout via a raw poll on the fd.
    let r = resp_rx;
    let mut pollfd = libc::pollfd {
        fd: r.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let n = unsafe { libc::poll(&mut pollfd, 1, 8000) };
    if n > 0 {
        use std::io::Read;
        let mut buf = [0u8; 4096];
        let mut r = r;
        match r.read(&mut buf) {
            Ok(0) => println!("EOF: browser closed the pipe"),
            Ok(k) => println!("RESPONSE: {}", String::from_utf8_lossy(&buf[..k])),
            Err(e) => println!("read err: {e}"),
        }
    } else {
        println!("poll timeout: {n}");
    }

    let _ = child.kill();
}
