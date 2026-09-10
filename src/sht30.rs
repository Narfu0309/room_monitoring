use std::{thread, time::Duration};

use anyhow::{Result, ensure};
use crc::{CRC_8_NRSC_5, Crc};
use esp_idf_svc::hal::i2c::I2cDriver;

pub struct Sht30<'a> {
    driver: I2cDriver<'a>,
    address: u8,
}

pub struct TemperatureAndHumidity {
    pub celsius: f32,
    pub humidity: f32,
}

impl<'a> Sht30<'a> {
    pub fn new(driver: I2cDriver<'a>, address: u8) -> Self {
        Self {
            driver,
            address,
        }
    }

    pub fn calc_crc(data: &[u8; 2]) -> u8 {
        let crc = Crc::<u8>::new(&CRC_8_NRSC_5);
        let mut digest = crc.digest();
        digest.update(data);
        digest.finalize()
    }

    pub fn measure_raw(&mut self, buf: &mut [u8; 6]) {
        self.driver.write(self.address, &[0x2C, 0x06], 1000).unwrap();
        thread::sleep(Duration::from_millis(15));
        self.driver.read(self.address, buf, 1000).unwrap();
    }

    /// An old value might be returned. [`Self::measure_accurately`]
    pub fn measure(&mut self) -> Result<TemperatureAndHumidity> {
        let mut data = [0u8; 6];
        self.measure_raw(&mut data);

        let temp_data: [u8; 2] = data[0..=1].try_into().unwrap();
        let temp_crc = data[2];
        ensure!(Self::calc_crc(&temp_data) == temp_crc);

        let humi_data: [u8; 2] = data[3..=4].try_into().unwrap();
        let humi_crc = data[5];
        ensure!(Self::calc_crc(&humi_data) == humi_crc);

        // https://sensirion.com/media/documents/213E6A3B/63A5A569/Datasheet_SHT3x_DIS.pdf#page=14
        Ok(TemperatureAndHumidity {
            celsius: -45. + 175. * (u16::from_be_bytes(temp_data) as f32 / ((1 << 16) - 1) as f32),
            humidity: 100. * (u16::from_be_bytes(humi_data) as f32 / ((1 << 16) - 1) as f32),
        })
    }

    /// To account for the possibility of receiving a stale value,
    /// the measurement is performed twice. This takes twice as long.
    pub fn measure_accurately(&mut self) -> Result<TemperatureAndHumidity> {
        self.measure()?;
        self.measure()
    }
}