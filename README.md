# ceti-stream

`ceti-stream` is an server daemon to CETI whale tags that forwards sensor data on UDP ports.

## TCP Commands
The server daemon consists of a tcp listener hosted on port `8080` to handle incoming requests. Requests are sent as an ascii string and available commands are as follows:
```
audio subscribe <udp_port>     - Subscribe your udp port to the raw audio data stream
audio unsubscribe <udp_port>   - Unsubscribe your udp port from the raw audio data stream
// todo: add remaining subscription commands
// todo: add pipe command forwarding
stop                           - kill the server
```

## UDP data formats
Except for audio and ecg, all data samples are sent as Plain Old Data (POD) with C structure representation as defined in [ceti.rs](src/daemon/ceti.rs). 
Data is transferred in the tag's native endianess (little-endian) not network endianess (big-endian). All data samples start with an i64 representing their unix time timestamp
for packet reordering.

Audio: Unlike the other data, audio is sent in big-endian ordering.
The first 4-bytes of the packet represent the packet's index as a u32. 
The index is incremented for each consecutive packet sent and wraps back to zero if 4,294,967,295 packets have been sent.
The remaining 1494 bytes represents 249 samples of audio. A sample consists of 3 interleaved channels of 16-bit audio captured at a sample rate of 96 kSPS.
The packets are always sample (channel and byte) aligned, to avoid data misalignment if packets are lost.

Ecg streaming is not yet implemented.

## Use
Cross compile binary (or download release) and move onto whale tag.
Guarentee `ceti-tag-data-capture.service` is running on the tag prior to running the server, to ensure that the sensor data shared memory objects exist and are being populated. 

## Cross Compiling
1) install rust

    ``` curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh ```

1) install raspberry pi cross compiliation toolchain following the instructions [here](https://github.com/tttapa/docker-arm-cross-toolchain) for `aarch64` architecture.
1) add platform target to rustc
   
     ``` rustup target add aarch64-unknown-linux-gnu ```
   
1) build with cargo

     ``` cargo build --bin daemon --target aarch64-unknown-linux-gnu ```

## Offline worker-reporting tests

Run these commands from the repository root with a host Rust compiler and linker
installed (`rustc` on `PATH`, or `RUSTC` set for the compiler used by the startup
smoke test). No Cargo dependencies or whale-tag hardware are needed.

On Linux/macOS:

```sh
rustc --edition=2021 --test -D warnings tests/worker_reporting.rs -o worker_reporting_test
./worker_reporting_test
```

On Windows (PowerShell, with a linker available for the installed Rust toolchain):

```powershell
rustc --edition=2021 --test -D warnings tests/worker_reporting.rs -o worker_reporting_test.exe
.\worker_reporting_test.exe
```

Expected result: five tests pass and one subprocess fixture is intentionally
ignored. The fixture is exercised by its parent test; do not run it separately
with `--ignored`. The suite compiles a synthetic daemon and uses temporary files
and an ephemeral loopback TCP port. It checks worker-error reporting and control
responsiveness with synthetic hardware workers, not real tag hardware,
shared-memory/semaphore behavior, Linux/ARM runtime, or graceful daemon shutdown.

## ToDo
- wrap daemon executable in systemd service with  `ceti-tag-data-capture.service` as a requirement.
- finish implementing tcp commands
- implement ecg sample forwarding
- create docker image for consistent build environment
- create installation script
