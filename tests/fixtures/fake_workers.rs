// Hardware stand-ins for the production startup smoke test. Reporting belongs
// to the real spawn sites and worker.rs, never these synthetic worker bodies.
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

type Subscribers = Arc<Mutex<Vec<SocketAddr>>>;
type StopFlag = Arc<Mutex<bool>>;

pub mod audio {
    use super::*;

    pub fn tx_thread(_: Subscribers, _: StopFlag) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::ConnectionReset,
            "injected failure",
        ))
    }
}

pub use audio as ecg;

pub mod fwd_thread {
    use super::*;
    use std::ffi::c_char;
    use std::time::Duration;

    pub struct ForwardThread {
        pub name: &'static str,
        pub shm_name: *const c_char,
        pub sem_name: *const c_char,
        pub size: usize,
        pub sample_period_us: Duration,
    }

    impl ForwardThread {
        pub fn create(&self) -> impl Fn(Subscribers, StopFlag) -> io::Result<()> + '_ {
            audio::tx_thread
        }
    }
}
