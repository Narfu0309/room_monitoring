use esp_idf_svc::{nvs::{EspDefaultNvs, EspNvsPartition, NvsDefault}, sys::EspError};

use crate::serial::Serial;

pub struct AppNvs {
    pub name: String,
    pub nvs: EspDefaultNvs,
}

impl AppNvs {
    pub fn take(nvs_part: EspNvsPartition<NvsDefault>, name: String) -> Result<AppNvs, EspError> {
        Ok(Self {
            nvs: EspDefaultNvs::new(nvs_part, &name, true)?,
            name,
        })
    }

    pub fn get_str(&self, key: &str, buf: &mut [u8], input: &mut Serial) -> String {
        self.nvs.get_str(key, buf).unwrap().map(|value| value.to_string()).unwrap_or_else(|| {
            println!("NVS の {0} に {1} が見つかりませんでした。{1} を入力してください。", self.name, key);
            let value = String::from_utf8_lossy(&input.readln()).to_string();
            self.nvs.set_str(key, &value).unwrap();
            value
        })
    }
}