use std::io::{self, Write};
use std::thread::{self, JoinHandle};

/// Report returned errors before the main thread reaches this worker's join.
/// Keep the original result and leave shutdown decisions to the caller.
pub fn spawn<F>(name: &'static str, worker: F) -> JoinHandle<io::Result<()>>
where
    F: FnOnce() -> io::Result<()> + Send + 'static,
{
    thread::spawn(move || {
        let result = worker();
        if let Err(error) = &result {
            // A failed diagnostic must not replace the worker's original error.
            let _ = writeln!(io::stderr().lock(), "{name} thread failed: {error}");
        }
        result
    })
}
