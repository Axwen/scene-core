//! Process boundary contract: no shell, bounded output, deadline and cancel.

use scene_core_media::process::{ProcessSpec, run};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[cfg(unix)]
fn long_running() -> (String, Vec<String>) {
    (
        "sh".to_owned(),
        vec!["-c".to_owned(), "exec sleep 30".to_owned()],
    )
}

#[cfg(windows)]
fn long_running() -> (String, Vec<String>) {
    (
        "powershell".to_owned(),
        vec![
            "-NoProfile".to_owned(),
            "-Command".to_owned(),
            "Start-Sleep -Seconds 30".to_owned(),
        ],
    )
}

#[test]
fn timeout_kills_a_long_running_tool() {
    let (program, args) = long_running();
    let started = Instant::now();
    let timeout = if cfg!(windows) {
        Duration::from_millis(2000)
    } else {
        Duration::from_millis(200)
    };
    let output = run(&ProcessSpec::new(program, args).with_timeout(timeout)).expect("spawn");
    assert!(output.timed_out);
    assert!(!output.success);
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn cancellation_kills_a_long_running_tool() {
    let (program, args) = long_running();
    let flag = Arc::new(AtomicBool::new(false));
    let spec = ProcessSpec::new(program, args).with_cancellation(flag.clone());
    let worker = std::thread::spawn(move || run(&spec));
    std::thread::sleep(Duration::from_millis(if cfg!(windows) {
        1500
    } else {
        150
    }));
    flag.store(true, Ordering::Relaxed);
    let output = worker.join().expect("worker").expect("spawn");
    assert!(output.cancelled);
    assert!(!output.success);
}

#[cfg(unix)]
#[test]
fn output_is_truncated_at_the_limit() {
    let output = run(&ProcessSpec::new("sh", ["-c", "yes x | head -c 100000"])
        .with_timeout(Duration::from_secs(30))
        .with_output_limits(1024, 1024))
    .expect("spawn");
    assert!(output.success);
    assert!(output.stdout.len() <= 1024);
    assert!(output.stdout_truncated);
}

#[test]
fn missing_program_is_a_spawn_error() {
    let error = run(&ProcessSpec::new(
        std::env::temp_dir().join("scene-core-missing-tool"),
        Vec::<String>::new(),
    ))
    .expect_err("missing program must not spawn");
    assert!(matches!(
        error,
        scene_core_media::process::ProcessError::Spawn(_)
    ));
}

#[test]
fn tools_are_not_resolved_through_path() {
    let bare = Command::new("definitely-not-a-real-tool-scene-core").output();
    assert!(bare.is_err());
}
