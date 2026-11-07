// The reporting boundary uses only std, so this can also run without tag hardware:
// rustc --edition=2021 --test tests/worker_reporting.rs -o worker_reporting
// The startup smoke test also needs a host compiler/linker (RUSTC or rustc on PATH).
#[path = "../src/daemon/worker.rs"]
mod worker;

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(10);

fn lines(reader: impl Read + Send + 'static) -> Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            let _ = sender.send(line.unwrap());
        }
    });
    receiver
}

struct Fixture(Child);

impl Fixture {
    fn start() -> (Self, SocketAddr) {
        Self::start_command(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "worker_process",
                    "--nocapture",
                    "--format=terse",
                ])
                .env("CETI_WORKER_REPORTING_TEST", "1"),
            "LISTENING ",
        )
    }

    fn start_command(command: &mut Command, prefix: &str) -> (Self, SocketAddr) {
        let child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut fixture = Self(child);
        let output = lines(fixture.0.stdout.take().unwrap());
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let line = output
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("fixture did not start");
            if let Some(address) = line.strip_prefix(prefix) {
                return (fixture, address.parse().unwrap());
            }
        }
    }

    fn finish(&mut self) {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                assert!(status.success(), "fixture failed: {status}");
                return;
            }
            assert!(Instant::now() < deadline, "fixture did not shut down");
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn command(address: SocketAddr, command: &str) {
    assert_eq!(request(address, command), "ok\n");
}

fn request(address: SocketAddr, command: &str) -> String {
    let mut stream = TcpStream::connect_timeout(&address, TIMEOUT).unwrap();
    stream.set_read_timeout(Some(TIMEOUT)).unwrap();
    stream.set_write_timeout(Some(TIMEOUT)).unwrap();
    // Keep the complete command in one buffer for the handler's single read.
    stream.write_all(format!("{command}\n").as_bytes()).unwrap();
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).unwrap();
    reply
}

#[test]
fn production_startup_reports_every_stream_failure() {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Sources(std::path::PathBuf);
    impl Drop for Sources {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let path = std::env::temp_dir().join(format!(
        "ceti-worker-startup-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&path).unwrap();
    let sources = Sources(path);

    // Compile the real startup and control code. Substitute only hardware bodies,
    // C scalar aliases and the listening address; keep every production spawn/join.
    let files = [
        (
            "main.rs",
            include_str!("../src/daemon/main.rs"),
            vec![
                ("mod audio;", "mod fake_workers; use fake_workers::audio;"),
                ("mod ecg;", "use fake_workers::ecg;"),
                ("mod fwd_thread;", "use fake_workers::fwd_thread;"),
            ],
        ),
        (
            "ceti.rs",
            include_str!("../src/daemon/ceti.rs"),
            vec![(
                "use libc::{c_char, c_int};",
                "use std::ffi::{c_char, c_int};",
            )],
        ),
        (
            "subscription.rs",
            include_str!("../src/daemon/subscription.rs"),
            vec![("0.0.0.0:8080", "127.0.0.1:0")],
        ),
        ("worker.rs", include_str!("../src/daemon/worker.rs"), vec![]),
        (
            "fake_workers.rs",
            include_str!("fixtures/fake_workers.rs"),
            vec![],
        ),
    ];
    for (name, source, replacements) in files {
        let mut source = source.to_owned();
        for (from, to) in replacements {
            assert_eq!(
                source.matches(from).count(),
                1,
                "fixture seam changed: {name}: {from}"
            );
            source = source.replace(from, to);
        }
        fs::write(sources.0.join(name), source).unwrap();
    }
    let executable = sources
        .0
        .join(format!("daemon{}", std::env::consts::EXE_SUFFIX));
    let build = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .arg("--edition=2021")
        .arg(sources.0.join("main.rs"))
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let (mut fixture, address) =
        Fixture::start_command(&mut Command::new(executable), "Server listening on ");
    let errors = lines(fixture.0.stderr.take().unwrap());
    let mut expected = [
        "Accelerometer",
        "Audio",
        "Battery",
        "ECG",
        "Gyroscope",
        "Light",
        "Magnetometer",
        "Pressure",
        "Quaternion",
    ]
    .map(|name| format!("{name} thread failed: injected failure"));
    let deadline = Instant::now() + TIMEOUT;
    let mut reports = Vec::new();
    for _ in &expected {
        reports.push(
            errors
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("production streaming worker did not report its failure"),
        );
    }
    reports.sort();
    expected.sort();
    assert_eq!(reports, expected);
    assert_eq!(request(address, "help"), "command options:\n");
    assert!(fixture.0.try_wait().unwrap().is_none());
    // Drop terminates/reaps the synthetic daemon. Production stop behavior is
    // deliberately unchanged, so this test makes no graceful-shutdown claim.
}

#[test]
fn errors_are_reported_before_control_or_earlier_workers_are_joined() {
    let (mut fixture, address) = Fixture::start();
    let errors = lines(fixture.0.stderr.take().unwrap());
    command(address, "fail");

    let mut reports = [
        errors
            .recv_timeout(TIMEOUT)
            .expect("audio/sensor error delayed"),
        errors
            .recv_timeout(TIMEOUT)
            .expect("audio/sensor error delayed"),
    ];
    reports.sort();
    assert_eq!(reports[0], "Audio thread failed: injected audio failure");
    assert!(reports[1].starts_with("Battery thread failed: "));
    assert!(fixture.0.try_wait().unwrap().is_none());
    command(address, "ping");
    command(address, "stop");
    fixture.finish();
    // Successful workers stay silent, and joining does not report errors again.
    assert_eq!(
        errors.recv_timeout(TIMEOUT),
        Err(mpsc::RecvTimeoutError::Disconnected)
    );
}

#[test]
fn closed_stderr_does_not_replace_worker_errors() {
    let (mut fixture, address) = Fixture::start();
    drop(fixture.0.stderr.take());
    command(address, "fail");
    command(address, "stop");
    fixture.finish();
}

#[test]
fn panics_remain_join_errors() {
    let result = worker::spawn("Pressure", || panic!("injected worker panic")).join();
    assert_eq!(
        result.unwrap_err().downcast_ref::<&str>(),
        Some(&"injected worker panic")
    );
}

#[test]
fn successful_workers_wait_for_stop_and_return_ok() {
    let (stop, stopped) = mpsc::channel();
    let handle = worker::spawn("Light", move || {
        stopped.recv().unwrap();
        Ok(())
    });
    assert!(!handle.is_finished());
    stop.send(()).unwrap();
    handle.join().unwrap().unwrap();
}

// A hardware-free stand-in for the daemon's join order. The control worker uses
// loopback on an ephemeral port; this does not exercise subscription::tcp_handler.
#[test]
#[ignore = "subprocess fixture"]
fn worker_process() {
    assert_eq!(
        std::env::var("CETI_WORKER_REPORTING_TEST").as_deref(),
        Ok("1")
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = mpsc::channel();
    let (fail_audio, audio_failure) = mpsc::channel();
    let (fail_sensor, sensor_failure) = mpsc::channel();

    // An earlier worker stays alive until the explicit stop command.
    let healthy = worker::spawn("ECG", move || {
        stopped.recv().unwrap();
        Ok(())
    });
    let audio = worker::spawn("Audio", move || {
        audio_failure.recv().unwrap();
        Err(io::Error::new(
            io::ErrorKind::ConnectionReset,
            "injected audio failure",
        ))
    });
    let sensor = worker::spawn("Battery", move || {
        sensor_failure.recv().unwrap();
        Err(io::Error::from_raw_os_error(123))
    });

    let control = thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut command = String::new();
            BufReader::new(&stream).read_line(&mut command).unwrap();
            match command.trim() {
                "fail" => {
                    fail_audio.send(()).unwrap();
                    fail_sensor.send(()).unwrap();
                }
                "ping" => {}
                "stop" => {
                    stop.send(()).unwrap();
                    stream.write_all(b"ok\n").unwrap();
                    break;
                }
                other => panic!("unexpected command: {other}"),
            }
            stream.write_all(b"ok\n").unwrap();
        }
    });
    println!("LISTENING {address}");
    io::stdout().flush().unwrap();

    control.join().unwrap();
    healthy.join().unwrap().unwrap();
    let error = audio.join().unwrap().unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::ConnectionReset);
    assert_eq!(error.to_string(), "injected audio failure");
    assert_eq!(
        sensor.join().unwrap().unwrap_err().raw_os_error(),
        Some(123)
    );
}
