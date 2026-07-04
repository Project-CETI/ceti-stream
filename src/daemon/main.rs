use std::env;
use std::mem::size_of;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::thread;

use fwd_thread::ForwardThread;

// Goals of this test program:
// [X] setup a UDP stream
// [X] send a raw audio file to the client
// [X] allow clients to subscribe to streams via tcp
// [X] stop stream when now subscribers
// [X] test on pi
// [X] access shm object on pi
// [ ] repeat for every sensor

mod audio;
pub mod ceti;
mod ecg;
mod fwd_thread;
mod subscription;

use ceti::*;

const BATTERY_FORWARDER: ForwardThread = ForwardThread {
    name: "Battery",
    shm_name: BATTERY_SHM_NAME,
    sem_name: BATTERY_SEM_NAME,
    size: size_of::<CetiBatterySample>(),
    sample_period_us: BATTERY_SAMPLING_PERIOD,
};

const LIGHT_FORWARDER: ForwardThread = ForwardThread {
    name: "Light",
    shm_name: LIGHT_SHM_NAME,
    sem_name: LIGHT_SEM_NAME,
    size: size_of::<CetiLightSample>(),
    sample_period_us: LIGHT_SAMPLING_PERIOD,
};

const PRESSURE_FORWARDER: ForwardThread = ForwardThread {
    name: "Pressure",
    shm_name: PRESSURE_SHM_NAME,
    sem_name: PRESSURE_SEM_NAME,
    size: size_of::<CetiPressureSample>(),
    sample_period_us: PRESSURE_SAMPLING_PERIOD,
};

const IMU_QUAT_FORWARDER: ForwardThread = ForwardThread {
    name: "Quaternion",
    shm_name: IMU_QUAT_SHM_NAME,
    sem_name: IMU_QUAT_SEM_NAME,
    size: size_of::<CetiImuQuatSample>(),
    sample_period_us: IMU_QUATERNION_SAMPLE_PERIOD_US,
};

const IMU_ACCEL_FORWARDER: ForwardThread = ForwardThread {
    name: "Accelerometer",
    shm_name: IMU_ACCEL_SHM_NAME,
    sem_name: IMU_ACCEL_SEM_NAME,
    size: size_of::<CetiImuAccelSample>(),
    sample_period_us: IMU_9DOF_SAMPLE_PERIOD_US,
};

const IMU_GYRO_FORWARDER: ForwardThread = ForwardThread {
    name: "Gyroscope",
    shm_name: IMU_GYRO_SHM_NAME,
    sem_name: IMU_GYRO_SEM_NAME,
    size: size_of::<CetiImuGyroSample>(),
    sample_period_us: IMU_9DOF_SAMPLE_PERIOD_US,
};

const IMU_MAG_FORWARDER: ForwardThread = ForwardThread {
    name: "Magnetometer",
    shm_name: IMU_MAG_SHM_NAME,
    sem_name: IMU_MAG_SEM_NAME,
    size: size_of::<CetiImuMagSample>(),
    sample_period_us: IMU_9DOF_SAMPLE_PERIOD_US,
};

fn main() -> std::io::Result<()> {
    let audio_sub_address: Arc<Mutex<Vec<SocketAddr>>> = Arc::new(Mutex::new(Vec::new()));
    let battery_sub_address: Arc<Mutex<Vec<SocketAddr>>> = Arc::new(Mutex::new(Vec::new()));
    let ecg_sub_address: Arc<Mutex<Vec<SocketAddr>>> = Arc::new(Mutex::new(Vec::new()));
    let imu_quat_sub_address: Arc<Mutex<Vec<SocketAddr>>> = Arc::new(Mutex::new(Vec::new()));
    let imu_accel_sub_address: Arc<Mutex<Vec<SocketAddr>>> = Arc::new(Mutex::new(Vec::new()));
    let imu_gyro_sub_address: Arc<Mutex<Vec<SocketAddr>>> = Arc::new(Mutex::new(Vec::new()));
    let imu_mag_sub_address: Arc<Mutex<Vec<SocketAddr>>> = Arc::new(Mutex::new(Vec::new()));
    let light_sub_address: Arc<Mutex<Vec<SocketAddr>>> = Arc::new(Mutex::new(Vec::new()));
    let pressure_sub_address: Arc<Mutex<Vec<SocketAddr>>> = Arc::new(Mutex::new(Vec::new()));
    let stop_flag: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

    let path = env::current_dir()?;
    println!("The current directory is {}", path.display());

    // start audio udp streaming thread
    let data_stream_threads = vec![
        {
            let sub_addr = audio_sub_address.clone();
            let stop_flag = stop_flag.clone();
            thread::spawn(move || audio::tx_thread(sub_addr, stop_flag))
        },
        {
            let sub_addr = ecg_sub_address.clone();
            let stop_flag = stop_flag.clone();
            thread::spawn(move || ecg::tx_thread(sub_addr, stop_flag))
        },
        {
            let sub_addr = battery_sub_address.clone();
            let stop_flag = stop_flag.clone();
            thread::spawn(move || BATTERY_FORWARDER.create()(sub_addr, stop_flag))
        },
        {
            let sub_addr = imu_quat_sub_address.clone();
            let stop_flag = stop_flag.clone();
            thread::spawn(move || IMU_QUAT_FORWARDER.create()(sub_addr, stop_flag))
        },
        {
            let sub_addr = imu_accel_sub_address.clone();
            let stop_flag = stop_flag.clone();
            thread::spawn(move || IMU_ACCEL_FORWARDER.create()(sub_addr, stop_flag))
        },
        {
            let sub_addr = imu_gyro_sub_address.clone();
            let stop_flag = stop_flag.clone();
            thread::spawn(move || IMU_GYRO_FORWARDER.create()(sub_addr, stop_flag))
        },
        {
            let sub_addr = imu_mag_sub_address.clone();
            let stop_flag = stop_flag.clone();
            thread::spawn(move || IMU_MAG_FORWARDER.create()(sub_addr, stop_flag))
        },
        {
            let sub_addr = light_sub_address.clone();
            let stop_flag = stop_flag.clone();
            thread::spawn(move || LIGHT_FORWARDER.create()(sub_addr, stop_flag))
        },
        {
            let sub_addr = pressure_sub_address.clone();
            let stop_flag = stop_flag.clone();
            thread::spawn(move || PRESSURE_FORWARDER.create()(sub_addr, stop_flag))
        },
    ];

    let subscription_thread = {
        let stop_flag = stop_flag.clone();
        let audio_sub_address = audio_sub_address.clone();
        let battery_sub_address = battery_sub_address.clone();
        let ecg_sub_address = ecg_sub_address.clone();
        let imu_quat_sub_address = imu_quat_sub_address.clone();
        let imu_accel_sub_address = imu_accel_sub_address.clone();
        let imu_gyro_sub_address = imu_gyro_sub_address.clone();
        let imu_mag_sub_address = imu_mag_sub_address.clone();
        let light_sub_address = light_sub_address.clone();
        let pressure_sub_address = pressure_sub_address.clone();
        thread::spawn(move || {
            subscription::tcp_handler(
                stop_flag,
                audio_sub_address,
                battery_sub_address,
                ecg_sub_address,
                imu_quat_sub_address,
                imu_accel_sub_address,
                imu_gyro_sub_address,
                imu_mag_sub_address,
                light_sub_address,
                pressure_sub_address,
            )
        })
    };

    subscription_thread.join().expect("thread panicked")?;
    println!("Waiting on \"stop\"");
    for thread in data_stream_threads {
        thread.join().expect("thread panicked")?;
    }

    println!("Goodbye!");

    Ok(())
}
