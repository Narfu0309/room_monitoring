use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use anyhow::{Result, bail};
use chrono::{NaiveTime, TimeDelta, Utc};
use chrono_humanize::{Accuracy, HumanTime, Tense};
use chrono_tz::Asia;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::gpio::{AnyInputPin, AnyOutputPin, PinDriver};
use esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::reset::WakeupReason;
use esp_idf_svc::hal::uart::{UartConfig, UartDriver};
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sntp::EspSntp;
use esp_idf_svc::wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi};
use room_monitoring::discord::{DiscordClient, handle_response};
use room_monitoring::{deep_sleep, next_measure_time, sec_to_nanos};
use room_monitoring::nvs::AppNvs;
use room_monitoring::serial::Serial;
use room_monitoring::sht30::Sht30;

#[allow(unused)]
const SDA: usize = 21;
#[allow(unused)]
const SCL: usize = 22;

const I2C_ADDRESS: u8 = 0x44;

const BAUD_RATE: u32 = 115200;

/// I have made a generous estimate;
/// any margin of error—at least up to 24 hours—should fall within this range.
const INTERNAL_RTC_TIME_ERROR_RATE: f64 = 0.2;
/// Deep Sleep will be used if waiting continues any longer.
const SECONDS_TO_USE_DEEP_SLEEP: i64 = 180;

/// It must be sorted.
const TIME_OF_MEASUREMENT: &[NaiveTime] = &[
    NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    NaiveTime::from_hms_opt(3, 0, 0).unwrap(),
    NaiveTime::from_hms_opt(6, 0, 0).unwrap(),
    NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
    NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
    NaiveTime::from_hms_opt(15, 0, 0).unwrap(),
    NaiveTime::from_hms_opt(16, 10, 0).unwrap(),
    NaiveTime::from_hms_opt(18, 0, 0).unwrap(),
    NaiveTime::from_hms_opt(21, 0, 0).unwrap(),
];

fn main() {
    // It is necessary to call this function once. Otherwise, some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();
    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    let mut buf = [0; 256];

    let peripherals = Peripherals::take().unwrap();

    let uart = UartDriver::new(
        peripherals.uart0,
        peripherals.pins.gpio1,
        peripherals.pins.gpio3,
        None::<AnyInputPin>,
        None::<AnyOutputPin>,
        &UartConfig::default()
    ).unwrap();
    let mut serial = Serial::new(uart);

    let sda = peripherals.pins.gpio21;
    let scl = peripherals.pins.gpio22;
    let i2c_config = I2cConfig::new().baudrate(BAUD_RATE.into());
    let i2c = I2cDriver::new(peripherals.i2c0, sda, scl, &i2c_config).unwrap();
    let mut sht30 = Sht30::new(i2c, I2C_ADDRESS);

    let sysloop = EspSystemEventLoop::take().unwrap();

    let nvs_part = EspDefaultNvsPartition::take().unwrap();
    let wifi_nvs = AppNvs::take(nvs_part.clone(), "wifi".to_string()).unwrap();

    let ssid = wifi_nvs.get_str("ssid", &mut buf, &mut serial);
    let pswd = wifi_nvs.get_str("pswd", &mut buf, &mut serial);

    let esp_wifi = EspWifi::new(peripherals.modem, sysloop.clone(), Some(nvs_part)).unwrap();
    let mut wifi = BlockingWifi::wrap(esp_wifi, sysloop).unwrap();

    let wifi_config = Configuration::Client(ClientConfiguration {
        auth_method: AuthMethod::WPA3Personal,
        ssid: (&ssid as &str).try_into().unwrap(),
        password: (&pswd as &str).try_into().unwrap(),
        ..ClientConfiguration::default()
    });
    wifi.set_configuration(&wifi_config).unwrap();

    wifi.start().unwrap();
    wifi.connect().unwrap();

    wifi.wait_netif_up().unwrap();

    println!("{:?}", wifi.wifi().sta_netif().get_ip_info().unwrap());

    println!("SNTP で時刻を同期しています...");
    let cond1 = Arc::new((Mutex::new(false), Condvar::new()));
    let cond2 = Arc::clone(&cond1);
    let _sntp = EspSntp::new_with_callback(&Default::default(), move |_dur| {
        let (lock, cvar) = &*cond2;
        let mut started = lock.lock().unwrap();
        *started = true;
        cvar.notify_all();
    }).unwrap();

    // Waiting for time synchronization.
    let (lock, cvar) = &*cond1;
    let mut started = lock.lock().unwrap();
    while !*started {
        started = cvar.wait(started).unwrap();
    }
    println!("時刻が同期されました: {}", Utc::now().with_timezone(&Asia::Tokyo));

    let mut discord_client = DiscordClient::new();

    let wakeup_reason = WakeupReason::get();
    println!("起動要因: {:?}", wakeup_reason);
    if wakeup_reason == WakeupReason::Unknown {
        println!("電源が手動で投入されたため、直ちに測定されます。");
        measurement_and_post(&mut discord_client, &mut sht30, &mut buf, Some(50)).unwrap();
    }

    loop {
        let mut current_time = Utc::now().with_timezone(&Asia::Tokyo);
        let actual_expected_time = next_measure_time(TIME_OF_MEASUREMENT).unwrap();
        let adjusted_expected_time = actual_expected_time - TimeDelta::seconds(SECONDS_TO_USE_DEEP_SLEEP);

        println!("現在時刻: {}", current_time);
        println!("計測時刻: {}", actual_expected_time);

        if current_time < adjusted_expected_time {
            // We will wake up early to account for any margin of error.
            let waiting = {
                let mut delta_nanos = (adjusted_expected_time - current_time).num_nanoseconds().unwrap() as f64;
                // When the maximum error in the wake up time could result in the time exceeding the scheduled measurement time.
                if delta_nanos * INTERNAL_RTC_TIME_ERROR_RATE >= sec_to_nanos(SECONDS_TO_USE_DEEP_SLEEP as _)  {
                    // Adjustments will be made to ensure the timing aligns with the scheduled measurement time.
                    delta_nanos *= 1. - INTERNAL_RTC_TIME_ERROR_RATE;
                }
                TimeDelta::nanoseconds(delta_nanos.round() as _)
            };
            println!("Deep Sleep で次の時間待機します: {}", HumanTime::from(waiting).to_text_en(Accuracy::Precise, Tense::Present));
            let notification_led = PinDriver::output(peripherals.pins.gpio2).unwrap();
            deep_sleep(notification_led, Some(&mut wifi), Some(waiting.num_microseconds().unwrap() as _));
        }

        loop {
            current_time = Utc::now().with_timezone(&Asia::Tokyo);
            if current_time >= actual_expected_time {
                break;
            }
            let waiting = actual_expected_time - current_time;
            println!("通常 Sleep で待機中: {}", HumanTime::from(waiting).to_text_en(Accuracy::Precise, Tense::Present));
            thread::sleep(waiting.to_std().unwrap());
        }

        measurement_and_post(&mut discord_client, &mut sht30, &mut buf, Some(100)).unwrap();
    }
}

pub fn measurement_and_post(client: &mut DiscordClient, sht30: &mut Sht30, buf: &mut [u8], number_of_allowable_failures: Option<usize>) -> Result<()> {
    let mut data = sht30.measure();
    let mut failure_count = 0;
    while data.is_err() {
        failure_count += 1;
        if let Some(n) = number_of_allowable_failures && failure_count > n {
            bail!("計測に {} 回失敗しました。", failure_count);
        }
        data = sht30.measure_accurately();
    }
    dbg!(failure_count);
    let data = data.unwrap();
    let formatted_data = format!("{}\n温度 {} ℃\n湿度 {} %", Utc::now().with_timezone(&Asia::Tokyo), data.celsius, data.humidity);
    println!("{}", formatted_data);

    let res = client.post(formatted_data);
    handle_response(res, buf);

    Ok(())
}