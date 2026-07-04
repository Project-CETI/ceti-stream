use std::time::Duration;

use libc::{c_char, c_int};

pub const BATTERY_SHM_NAME: *const c_char = c"/battery_shm".as_ptr();
pub const BATTERY_SEM_NAME: *const c_char = c"/battery_sem".as_ptr();
pub const BATTERY_SAMPLING_PERIOD: Duration = Duration::from_secs(1);

pub const ECG_NUM_BUFFER: usize = 2;
pub const ECG_BUFFER_LENGTH: usize = 10000;

pub const LIGHT_SHM_NAME: *const c_char = c"/light_shm".as_ptr();
pub const LIGHT_SEM_NAME: *const c_char = c"/light_sem".as_ptr();
pub const LIGHT_SAMPLING_PERIOD: Duration = Duration::from_secs(1);

pub const PRESSURE_SHM_NAME: *const c_char = c"/pressure_shm".as_ptr();
pub const PRESSURE_SEM_NAME: *const c_char = c"/pressure_sem".as_ptr();
pub const PRESSURE_SAMPLING_PERIOD: Duration = Duration::from_secs(1);

pub const IMU_QUAT_SHM_NAME: *const c_char = c"/imu_quat_shm".as_ptr();
pub const IMU_QUAT_SEM_NAME: *const c_char = c"/imu_quat_sample_sem".as_ptr();
pub const IMU_QUATERNION_SAMPLE_PERIOD_US: Duration = Duration::from_millis(5); // rate for the computed orientation

pub const IMU_ACCEL_SHM_NAME: *const c_char = c"/imu_accel_shm".as_ptr();
pub const IMU_ACCEL_SEM_NAME: *const c_char = c"/imu_accel_sample_sem".as_ptr();
pub const IMU_GYRO_SHM_NAME: *const c_char = c"/imu_gyro_shm".as_ptr();
pub const IMU_GYRO_SEM_NAME: *const c_char = c"/imu_gyro_sample_sem".as_ptr();
pub const IMU_MAG_SHM_NAME: *const c_char = c"/imu_mag_shm".as_ptr();
pub const IMU_MAG_SEM_NAME: *const c_char = c"/imu_mag_sample_sem".as_ptr();
pub const IMU_9DOF_SAMPLE_PERIOD_US: Duration = Duration::from_millis(2); // rate for the accelerometer/gyroscope/magnetometer

#[derive(Debug, Clone)]
#[repr(C)]
pub struct CetiBatterySample {
    pub sys_time_us: i64,
    pub error: i32,
    pub rtc_time_s: c_int,
    pub cell_voltage_v: [f64; 2],
    pub cell_temperature_c: [f64; 2],
    pub current_ma: f64,
    pub state_of_charge: f64,
    pub status: u16,
    pub protection_alert: u16,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct CetiEcgSample {
    pub sys_time_us: u64,
    pub sample_index: u64,
    pub error: i32,
    pub rtc_time_s: u32,
    pub ecg_reading: i32,
    pub leads_off_reading_n: u16,
    pub leads_off_reading_p: u16,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct CetiEcgBuffer {
    pub page: c_int,   // which buffer will be populated with new incoming data
    pub sample: c_int, // which sample will be populated with new incoming data
    pub lod_enabled: c_int,
    pub data: [CetiEcgSample; ECG_BUFFER_LENGTH * ECG_NUM_BUFFER],
}

#[repr(C)]
pub struct CetiImuQuatSample {
    pub sys_time_us: i64,
    pub reading_delay_us: i64,
    pub rtc_time_s: c_int,
    pub i: i16,
    pub j: i16,
    pub k: i16,
    pub real: i16,
    pub accuracy: i16,
}

#[repr(C)]
pub struct CetiImuAccelSample {
    pub sys_time_us: i64,
    pub reading_delay_us: i64,
    pub rtc_time_s: c_int,
    pub x: i16,
    pub y: i16,
    pub z: i16,
    pub accuracy: i16,
}

#[repr(C)]
pub struct CetiImuGyroSample {
    pub sys_time_us: i64,
    pub reading_delay_us: i64,
    pub rtc_time_s: c_int,
    pub x: i16,
    pub y: i16,
    pub z: i16,
    pub accuracy: i16,
}

#[repr(C)]
pub struct CetiImuMagSample {
    sys_time_us: i64,
    reading_delay_us: i64,
    rtc_time_s: c_int,
    x: i16,
    y: i16,
    z: i16,
    accuracy: i16,
}

#[repr(C)]
pub struct CetiLightSample {
    pub sys_time_us: i64,
    pub rtc_time_s: c_int,
    pub error: i32,
    pub visible: c_int,
    pub infrared: c_int,
}

#[repr(C)]
pub struct CetiPressureSample {
    sys_time_us: i64,
    rtc_time_s: c_int,
    error: i32,
    pressure_bar: f64,
    temperature_c: f64,
}
