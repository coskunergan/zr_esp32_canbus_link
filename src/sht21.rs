// Copyright (c) 2025
// SPDX-License-Identifier: Apache-2.0
// Coskun ERGAN <coskunergan@gmail.com>

extern "C" {
    fn sht21_init() -> i32;
    fn sht21_read_data(temp: *mut f32, humi: *mut f32) -> i32;
}

pub struct Sht21 {
    _private: (),
}

impl Sht21 {
    pub fn new() -> Result<Self, i32> {
        let result = unsafe { sht21_init() };

        if result == 0 {
            Ok(Sht21 { _private: () })
        } else {
            Err(result)
        }
    }
    pub fn read_data(&self) -> Result<(f32, f32), i32> {
        let mut temperature: f32 = 0.0;
        let mut humidity: f32 = 0.0;

        let result = unsafe { sht21_read_data(&mut temperature, &mut humidity) };

        if result == 0 {
            Ok((temperature, humidity))
        } else {
            Err(result)
        }
    }
}
