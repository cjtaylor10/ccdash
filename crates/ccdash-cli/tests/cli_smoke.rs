use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tempfile::tempdir;

/// Spawn the daemon, then run the CLI binary against it.
#[test]
fn cli_status_round_trip() {
    let dir = tempdir().unwrap();
    let socket = dir.path().join("ccdash.sock");
    let data_dir = dir.path().join("home");
    std::fs::create_dir_all(&data_dir).unwrap();

    // CARGO_BIN_EXE_<name> is only set for binaries in the SAME crate as the test,
    // so we get the cli binary that way and compute the daemon binary path
    // relative to it (both live in target/<profile>/).
    let cli_bin = PathBuf::from(env!("CARGO_BIN_EXE_ccdash"));
    let daemon_bin = cli_bin
        .parent()
        .expect("cli bin has parent dir")
        .join("ccdash-daemon");
    assert!(
        cli_bin.exists(),
        "cli binary missing at {}",
        cli_bin.display()
    );
    assert!(
        daemon_bin.exists(),
        "daemon binary missing at {} — run `cargo build -p ccdash-daemon` first",
        daemon_bin.display()
    );

    let mut daemon = Command::new(&daemon_bin)
        .arg("--socket")
        .arg(&socket)
        .arg("--data-dir")
        .arg(&data_dir)
        .arg("--log-level")
        .arg("warn")
        .spawn()
        .expect("spawn daemon");

    // Wait for socket up.
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while !socket.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(socket.exists(), "daemon socket never appeared");

    let out = Command::new(&cli_bin)
        .arg("--socket")
        .arg(&socket)
        .arg("status")
        .env("CCDASH_HOME", &data_dir)
        .output()
        .expect("run cli");

    let _ = daemon.kill();
    let _ = daemon.wait();

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "cli failed: stdout={} stderr={}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("daemon: ok"),
        "unexpected stdout: {}",
        stdout
    );
}
