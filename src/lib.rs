use std::{thread, time::Duration};

use chrono::{DateTime, Days, NaiveTime, Utc};
use chrono_tz::{Asia, Tz};
use embedded_svc::wifi::Wifi;
use esp_idf_svc::{hal::gpio::{Output, PinDriver}, sys::{esp_deep_sleep, esp_deep_sleep_start}};

pub mod sht30;
pub mod serial;
pub mod nvs;
pub mod discord;

pub fn to_fahrenheit(celsius: f32) -> f32 {
    (celsius * 1.8) + 32.
}

/// If the current time is equal to the measurement time, the current time is returned.
pub fn next_measure_time(times: &[NaiveTime]) -> Option<DateTime<Tz>> {
    if times.is_empty() {
        return None;
    }

    let now = Utc::now().with_timezone(&Asia::Tokyo);
    let now_naive_date = now.date_naive();
    let now_naive_local = now.naive_local();

    if times.len() == 1 {
        return Some(
            now_naive_date
                .and_time(times[0])
                .and_local_timezone(Asia::Tokyo).unwrap()
        );
    }
    for i in 0..times.len() - 1 {
        let prev = now_naive_date.and_time(times[i]);
        let next = now_naive_date.and_time(times[i + 1]);
        if prev < now_naive_local && now_naive_local <= next {
            return Some(next.and_local_timezone(Asia::Tokyo).unwrap());
        }
    }
    Some(
        now_naive_date
            .checked_add_days(Days::new(1)).unwrap()
            .and_time(times[0])
            .and_local_timezone(Asia::Tokyo).unwrap()
    )
}

pub fn deep_sleep(mut led: PinDriver<'_, Output>, wifi: Option<&mut impl Wifi>, time_in_us: Option<u64>) -> ! {
    if let Some(wifi) = wifi {
        wifi.disconnect().unwrap();
        wifi.stop().unwrap();
    }

    led.set_high().unwrap();
    thread::sleep(Duration::from_secs(1));
    led.set_low().unwrap();

    if let Some(time_in_us) = time_in_us {
        unsafe {
            esp_deep_sleep(time_in_us);
        }
    } else {
        unsafe {
            esp_deep_sleep_start();
        }
    }
}

pub fn sec_to_nanos(sec: f64) -> f64 {
    sec * 1_000_000_000.
}