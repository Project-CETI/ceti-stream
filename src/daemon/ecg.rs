use libc::{c_char, off_t, sem_t};
use libc::{ftruncate, mmap, sem_open, sem_wait, shm_open};
use libc::{MAP_SHARED, O_RDWR, PROT_READ, S_IRUSR};
use std::mem::size_of;
use std::net::{SocketAddr, UdpSocket};
use std::ptr::{self};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use super::ceti::*;

const ECG_SHM_NAME: *const c_char = c"/ecg_shm".as_ptr();
const ECG_SAMPLE_SEM_NAME: *const c_char = c"/ecg_sample_sem".as_ptr();

const UDP_PACKET_SIZE_MAX: usize = 1500;
const SAMPLES_PER_PACKET: usize = UDP_PACKET_SIZE_MAX / size_of::<CetiEcgSample>();

pub fn tx_thread(
    dest_addr: Arc<Mutex<Vec<SocketAddr>>>,
    stop_flag: Arc<Mutex<bool>>,
) -> std::io::Result<()> {
    //open shared memory object
    let ecg_addr: *const CetiEcgBuffer = unsafe {
        let null = ptr::null_mut();
        let fd = shm_open(ECG_SHM_NAME, O_RDWR, S_IRUSR);
        let _res = ftruncate(fd, size_of::<CetiEcgBuffer>() as off_t);
        let addr = mmap(
            null,
            size_of::<CetiEcgBuffer>(),
            PROT_READ,
            MAP_SHARED,
            fd,
            0,
        );
        addr as *const CetiEcgBuffer
    };

    // open ecg sample semaphore
    let ecg_sem: *mut sem_t = unsafe { sem_open(ECG_SAMPLE_SEM_NAME, O_RDWR, S_IRUSR, 0) };

    //create Udp socket
    let socket = UdpSocket::bind("0.0.0.0:0")?;

    let local_address = socket.local_addr()?;
    println!("Server listening on {:?}", local_address);

    let mut stop = *stop_flag.lock().unwrap();
    let mut paused = true; // flag for if there are no subscribers
    let mut read_offset = 0;
    let mut write_offset = 0;

    println!("starting loop");
    while !stop {
        //check if any udp sockets are subscribed to audio stream.
        let dest_list = { (*dest_addr.lock().unwrap()).clone() };
        if !(*dest_list).is_empty() {
            // there is someone subscribed to the udp stream
            if paused {
                //unpause the udp stream

                // wait for new ecg sample to be posted
                // Note: This may wait forever better to include timeout
                unsafe {
                    let _res = sem_wait(ecg_sem);
                };
                let page = unsafe { (*ecg_addr).page } as usize;
                let sample = unsafe { (*ecg_addr).sample } as usize;
                write_offset = page * ECG_BUFFER_LENGTH + sample;
                read_offset = page * ECG_BUFFER_LENGTH;
                paused = false;
                println!("Starting ECG Stream");

                // println!("Offsets set to {:} and {:} [{:}][{:}]",read_offset, write_offset, page, sample);
            } else if read_offset == write_offset {
                unsafe {
                    let _res = sem_wait(ecg_sem);
                }; //get more data
                   //wait for more data if not enough in buffer
                let page = unsafe { (*ecg_addr).page } as usize;
                let sample = unsafe { (*ecg_addr).sample } as usize;
                write_offset = page * ECG_BUFFER_LENGTH + sample;
                // println!("{:?}", unsafe{(*ecg_addr).sample});
                // println!("Offsets set to {:} and {:} [{:}][{:}]",read_offset, write_offset, page, sample);
            }

            while read_offset != write_offset {
                //calculate number of new samples in buffer
                let mut sample_count = if read_offset < write_offset {
                    write_offset - read_offset
                } else {
                    // only precess to end of buffer if wrapping.
                    // Start of buffer will be picked up by next packet
                    (ECG_BUFFER_LENGTH * ECG_NUM_BUFFER) - read_offset
                };
                sample_count = sample_count.min(SAMPLES_PER_PACKET);

                // send packet
                let data = &unsafe { (*ecg_addr).data };
                let ecg_read_ptr = (&data[read_offset]) as *const CetiEcgSample;
                let packet: &[u8] = unsafe {
                    std::slice::from_raw_parts(
                        ecg_read_ptr as *const u8,
                        size_of::<CetiEcgSample>() * sample_count,
                    )
                };

                //send to all subscribed upd addresses
                for dest_addr in dest_list.iter() {
                    socket.send_to(packet, dest_addr)?;
                }

                // advance read head
                read_offset = (read_offset + sample_count) % (ECG_NUM_BUFFER * ECG_BUFFER_LENGTH);
            }
        } else {
            // there is noone subscribed to the udp stream
            if !paused {
                paused = true;
            }
            sleep(Duration::from_secs(1));
        }
        //update stop flag
        stop = *stop_flag.lock().unwrap();
    }

    println!("ECG Streaming thread has been stopped");
    Ok(())
}
