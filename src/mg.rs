// Copyright (c) 2025
// SPDX-License-Identifier: Apache-2.0
// Coskun ERGAN <coskunergan@gmail.com>

extern "C" {
    fn mongoose_init();
    fn mongoose_poll();
    fn glue_set_temperature(new_temp: i32);
    fn glue_set_humidity(new_humi: i32);
    fn glue_get_leds(data: *mut Leds);
}

#[repr(C)]
pub struct Leds {
    pub led1: bool,
    pub led2: bool,
    pub led3: bool,
}

pub struct Mongoose {
    _private: (),
}

impl Mongoose {
    pub fn new() -> Self {
        unsafe { mongoose_init() };
        Mongoose { _private: () }
    }
    pub fn mg_poll(&self) {
        unsafe { mongoose_poll() };
    }
    pub fn set_temperature(&self, temp: i32) {
        unsafe {
            glue_set_temperature(temp);
        }
    }
    pub fn set_humidity(&self, temp: i32) {
        unsafe {
            glue_set_humidity(temp);
        }
    }
    pub fn get_leds(&self, led: *mut Leds) {
        unsafe {
            glue_get_leds(led);
        }
    }
}
