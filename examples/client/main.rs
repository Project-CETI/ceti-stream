// ToDo:
// [ ] get hostname from cli args
// [ ]

use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, Sink};
use std::io::Write;
use std::net::{TcpStream, UdpSocket};
use std::thread::sleep;
use std::time::Duration;

const PACKET_SIZE_BYTES: usize = 1494;
const PACKET_SIZE_SAMPLES: usize = PACKET_SIZE_BYTES / AUDIO_SAMPLE_SIZE_BYTES;

const AUDIO_BUFFER_SIZE_MS: usize = 60000;
const AUDIO_SAMPLE_RATE_SPS: usize = 96000;
const AUDIO_MAX_CHANNELS: usize = 3;
const AUDIO_SAMPLE_SIZE_BYTES: usize = 2 * AUDIO_MAX_CHANNELS;
const AUDIO_BUFFER_MIN_SIZE_SAMPLES: usize = AUDIO_BUFFER_SIZE_MS * AUDIO_SAMPLE_RATE_SPS / 1000;
const AUDIO_BUFFER_SIZE_SAMPLES: usize =
    PACKET_SIZE_SAMPLES * AUDIO_BUFFER_MIN_SIZE_SAMPLES.div_ceil(PACKET_SIZE_SAMPLES);
const AUDIO_BUFFER_SIZE_BYTES: usize = AUDIO_BUFFER_SIZE_SAMPLES * AUDIO_SAMPLE_SIZE_BYTES;
const AUDIO_BUFFER_SIZE_PACKETS: usize = AUDIO_BUFFER_SIZE_BYTES / PACKET_SIZE_BYTES;

fn main() -> std::io::Result<()> {
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();

    // let mut raw_audio_buffer: [u8; AUDIO_BUFFER_SIZE_BYTES] = [0u8; AUDIO_BUFFER_SIZE_BYTES];

    let udp_socket = UdpSocket::bind("0.0.0.0:0").expect("couldn't bind to address");
    udp_socket.set_broadcast(false)?;
    println!("Binding to {:?}", udp_socket.local_addr().unwrap());

    let mut tcp_socket = TcpStream::connect("10.0.0.50:8080")?;
    println!("Binding to {:?}", udp_socket.local_addr().unwrap());
    // let mut sub_request = String::from("audio subscribe ");
    // sub_request.push_str(&udp_socket.local_addr().unwrap().port().to_string());
    // sub_request.push(' ');

    // println!("{:?}\n", sub_request);
    // tcp_socket.write(&sub_request.into_bytes())?;
    tcp_socket.write_fmt(format_args!(
        "audio subscribe {:?} ",
        udp_socket.local_addr().unwrap().port()
    ))?;

    let mut rx_buffer = [0; 1496];
    let mut start_index: Option<usize> = None;

    // tcp_socket.write(b"stop")?;
    println!("Starting loop");
    loop {
        let size = udp_socket.recv(&mut rx_buffer).expect("couldn't recv");
        if size == 0 {
            break;
        }

        //check header
        let packet_index =
            u32::from_le_bytes([rx_buffer[0], rx_buffer[1], rx_buffer[2], rx_buffer[3]]);
        let packet_size = size - 4;
        println!(
            "Received packet {:?} with {:?} bytes",
            packet_index, packet_size
        );
        //set buffer start index
        if start_index.is_none() {
            start_index = Some(packet_index as usize);
        }

        //assert_eq!(size, (packet_size as usize) + 4, "Size mismatch between packet size and received packet size");
        if packet_size == 0 {
            break;
        }

        //place in raw audio buffer
        let current_index = packet_index as usize - start_index.unwrap();
        if current_index == 0xFFFF {
            let mut sub_request: [u8; 10] = *b"unaudio:\0\0";
            sub_request[8..]
                .copy_from_slice(&udp_socket.local_addr().unwrap().port().to_be_bytes());
            tcp_socket.write_all(&sub_request)?;
            break;
        }

        if current_index == AUDIO_BUFFER_SIZE_PACKETS {
            break;
        }
        let processed_audio: Vec<i16> = rx_buffer[4..4 + packet_size]
            .chunks_exact(2)
            .map(|b| i16::from_be_bytes([b[0], b[1]]))
            // .map(|s| {s.saturating_mul(4)})
            .collect();
        let audio_source = SamplesBuffer::new(3, 96000, processed_audio);

        //append to sink
        sink.append(audio_source);
    }

    sleep(Duration::from_secs(5));
    tcp_socket.write_all(b"stop")?;
    println!("Goodbye!!");
    Ok(())
}
