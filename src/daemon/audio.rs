use libc::{c_char, c_int, off_t, sem_t};
use libc::{ftruncate, mmap, sem_open, sem_wait, shm_open};
use libc::{MAP_SHARED, O_RDWR, PROT_READ, S_IRUSR};
use std::mem::size_of;
use std::net::{SocketAddr, UdpSocket};
use std::ptr::{self};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

const AUDIO_SHM_NAME: *const c_char = c"/audio_shm".as_ptr();
const AUDIO_CHANNELS: usize = 3;
//multiple of AUDIO_LCM_BYTES closest to 75 seconds @ 16-bit, 96kSPS, (14401536)
const AUDIO_BUFFER_SIZE_BYTES_PER_CHANNEL: usize = 14401536;
const AUDIO_BUFFER_SIZE_BYTES: usize = AUDIO_CHANNELS * AUDIO_BUFFER_SIZE_BYTES_PER_CHANNEL;
const AUDIO_BUFFER_BLOCK_SIZE_BYTES: usize = 512 * 32;

const AUDIO_BLOCK_SEM_NAME: *const c_char = c"/audio_block_sem".as_ptr();
#[repr(C)]
struct CetiAudioBuffer {
    pub page: c_int,
    pub block: c_int,
    pub data: [u8; 2 * AUDIO_BUFFER_SIZE_BYTES],
}

pub fn tx_thread(
    dest_addr: Arc<Mutex<Vec<SocketAddr>>>,
    stop_flag: Arc<Mutex<bool>>,
) -> std::io::Result<()> {
    //open shared memory object
    let audio_addr = unsafe {
        let null = ptr::null_mut();
        let fd = shm_open(AUDIO_SHM_NAME, O_RDWR, S_IRUSR);
        let _res = ftruncate(fd, size_of::<CetiAudioBuffer>() as off_t);
        let addr = mmap(
            null,
            size_of::<CetiAudioBuffer>(),
            PROT_READ,
            MAP_SHARED,
            fd,
            0,
        );
        addr as *const CetiAudioBuffer
    };

    //open audio semaphore
    let audio_sem: *mut sem_t = unsafe { sem_open(AUDIO_BLOCK_SEM_NAME, O_RDWR, S_IRUSR, 0) };

    let socket = UdpSocket::bind("0.0.0.0:0")?;

    let local_address = socket.local_addr()?;
    println!("Server listening on {:?}", local_address);
    let mut buffer = [0; 1498];

    let mut packet_index: u32 = 0;
    let mut stop = *stop_flag.lock().unwrap();
    println!("Accessing semaphore",);
    let mut paused = true;
    let mut read_offset = 0;
    let mut write_offset = 0;
    while !stop {
        //check if any udp sockets are subscribed to audio stream.
        let dest_list = { (*dest_addr.lock().unwrap()).clone() };
        if !(*dest_list).is_empty() {
            if paused {
                unsafe {
                    let _res = sem_wait(audio_sem);
                };
                let page = unsafe { (*audio_addr).page as usize };
                let block = unsafe { (*audio_addr).block as usize };
                write_offset =
                    page * AUDIO_BUFFER_SIZE_BYTES + block * AUDIO_BUFFER_BLOCK_SIZE_BYTES;
                //align to full sample
                read_offset = write_offset.div_ceil(6);
                read_offset *= 6;
                assert_eq!(read_offset % 6, 0);

                //wait for next block
                let old_write = write_offset;
                while old_write == write_offset {
                    unsafe {
                        let _res = sem_wait(audio_sem);
                    }; //get more data
                       //wait for more data in not enough in buffer
                    let page = unsafe { (*audio_addr).page as usize };
                    let block = unsafe { (*audio_addr).block as usize };
                    write_offset =
                        page * AUDIO_BUFFER_SIZE_BYTES + block * AUDIO_BUFFER_BLOCK_SIZE_BYTES;
                }

                // println!("Audio Offsets set to {:} and {:} [{:}][{:}]",read_offset, write_offset, page, block);
                paused = false;
            }
            let mut remaining_bytes = 1494;
            //check if we need to read in more bytes
            if ((read_offset == write_offset)
                || (read_offset < write_offset) && ((write_offset - read_offset) < remaining_bytes))
                || ((2 * AUDIO_BUFFER_SIZE_BYTES - read_offset + write_offset) < remaining_bytes)
            {
                // println!("Grabbing audio...",);
                let old_write = write_offset;
                while old_write == write_offset {
                    unsafe {
                        let _res = sem_wait(audio_sem);
                    }; //get more data
                       //wait for more data in not enough in buffer
                    let page = unsafe { (*audio_addr).page as usize };
                    let block = unsafe { (*audio_addr).block as usize };
                    write_offset =
                        page * AUDIO_BUFFER_SIZE_BYTES + block * AUDIO_BUFFER_BLOCK_SIZE_BYTES;
                    if old_write != write_offset {
                        // println!("Audio Offsets set to {:} and {:} [{:}][{:}]",read_offset, write_offset, page, block);
                    }
                }
                continue;
            }

            buffer[0..4].copy_from_slice(&packet_index.to_le_bytes());

            //copy audio into buffer
            //check if wrapped
            if (write_offset < read_offset)
                && (2 * AUDIO_BUFFER_SIZE_BYTES - read_offset) < remaining_bytes
            {
                buffer[4..4 + (2 * AUDIO_BUFFER_SIZE_BYTES - read_offset)].copy_from_slice(
                    unsafe { &(&(*audio_addr).data)[read_offset..2 * AUDIO_BUFFER_SIZE_BYTES] },
                );
                remaining_bytes -= 2 * AUDIO_BUFFER_SIZE_BYTES - read_offset;
                read_offset = 0;
            }
            buffer[4 + (1494 - remaining_bytes)..].copy_from_slice(unsafe {
                &(&(*audio_addr).data)[read_offset..(read_offset + remaining_bytes)]
            });
            read_offset = (read_offset + remaining_bytes) % (2 * AUDIO_BUFFER_SIZE_BYTES);

            //send to all subscribed upd addresses
            for dest_addr in dest_list.iter() {
                socket.send_to(&buffer, dest_addr)?;
            }

            //increment packet index
            packet_index += 1;
        } else {
            if !paused {
                paused = true;
            }
            // println!("Waiting on subscribers");
            sleep(Duration::from_secs(2));
        }
        //update stop flag
        stop = *stop_flag.lock().unwrap();
    }
    println!("Audio Streaming thread has been stopped");
    Ok(())
}
