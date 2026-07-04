use libc::{c_char, off_t, sem_t};
use libc::{close, ftruncate, mmap, sem_open, sem_wait, shm_open};
use libc::{MAP_FAILED, MAP_SHARED, O_RDWR, PROT_READ, SEM_FAILED, S_IRUSR};
use std::mem::transmute;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{io, ptr};

pub type SubscriberList = Arc<Mutex<Vec<SocketAddr>>>;

pub struct ForwardThread {
    pub name: &'static str,
    pub shm_name: *const c_char,
    pub sem_name: *const c_char,
    pub size: usize,
    pub sample_period_us: Duration,
}

impl ForwardThread {
    fn open_shared_memory(&self) -> Result<*const u8, io::Error> {
        unsafe {
            let null = ptr::null_mut();
            let fd = shm_open(self.shm_name, O_RDWR, S_IRUSR);
            if fd == -1 {
                return Err(io::Error::last_os_error());
            }

            let res = ftruncate(fd, self.size as off_t);
            if res == -1 {
                return Err(io::Error::last_os_error());
            }

            let addr = mmap(null, self.size, PROT_READ, MAP_SHARED, fd, 0);
            if addr == MAP_FAILED {
                return Err(io::Error::last_os_error());
            }

            let _res = close(fd);
            Ok(addr as *const u8)
        }
    }

    fn open_semaphore(&self) -> Result<*mut sem_t, io::Error> {
        unsafe {
            let sem = sem_open(self.sem_name, O_RDWR, S_IRUSR, 0);
            if sem == SEM_FAILED {
                return Err(io::Error::last_os_error());
            }
            Ok(sem)
        }
    }

    pub fn create(&self) -> impl Fn(SubscriberList, Arc<Mutex<bool>>) -> std::io::Result<()> + '_ {
        |dest_addr, stop_flag| {
            //create udp socket
            let socket = UdpSocket::bind("0.0.0.0:0")?;
            let local_address = socket.local_addr()?;
            println!("Server transmitting {:?} on {:?}", self.name, local_address);

            // open shared memory object
            let shm_ptr = self.open_shared_memory()?;

            // open semaphore
            let data_ready: *mut sem_t = self.open_semaphore()?;

            //create main loop
            let mut stop = *stop_flag.lock().unwrap();
            while !stop {
                let mut last_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
                let dest_list = { (*dest_addr.lock().unwrap()).clone() };
                if !dest_list.is_empty() {
                    // get sample
                    unsafe {
                        let _res = sem_wait(data_ready);
                    };
                    let sample_systime_us: u64 =
                        unsafe { *transmute::<*const u8, *const u64>(shm_ptr) };
                    last_timestamp = Duration::from_micros(sample_systime_us);

                    //transmit sample to subscribers
                    //pack sample for transmission;
                    let tx_buffer = unsafe { std::slice::from_raw_parts(shm_ptr, self.size) };

                    //send messages
                    for dest_addr in dest_list.iter() {
                        socket.send_to(tx_buffer, dest_addr)?;
                    }
                }

                //sleep until next sample
                stop = *stop_flag.lock().unwrap();
                if stop {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
                    let elapsed_time = now - last_timestamp;
                    if elapsed_time < self.sample_period_us {
                        sleep(self.sample_period_us - elapsed_time);
                    }
                }
                stop = *stop_flag.lock().unwrap();
            }
            println!("{:?} thread complete!", self.name);
            Ok(())
        }
    }
}
