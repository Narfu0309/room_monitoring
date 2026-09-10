use esp_idf_svc::hal::uart::UartDriver;

pub struct Serial<'a> {
    uart: UartDriver<'a>,
}

impl<'a> Serial<'a> {
    pub fn new(uart: UartDriver<'a>) -> Self {
        Self {
            uart
        }
    }

    pub fn read(&self) -> Option<u8> {
        if self.uart.remaining_read().is_ok_and(|n| n != 0) {
            let mut buf = [0];
            self.uart.read(&mut buf, 1000).unwrap();
            Some(buf[0])
        } else {
            None
        }
    }

    pub fn read_buf(&self) -> Option<Vec<u8>> {
        if self.uart.remaining_read().is_ok_and(|n| n != 0) {
            let mut buf = vec![0; self.uart.remaining_read().unwrap()];
            self.uart.read(&mut buf, 1000).unwrap();
            Some(buf)
        } else {
            None
        }
    }

    pub fn readln(&self) -> Vec<u8> {
        let mut res = Vec::new();
        loop {
            if let Some(c) = self.read() {
                if c == b'\r' || c == b'\n' {
                    return res;
                }
                res.push(c);
            } else {
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
    }
}