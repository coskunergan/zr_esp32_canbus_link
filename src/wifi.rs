// Copyright (c) 2025
// SPDX-License-Identifier: Apache-2.0
// Coskun ERGAN <coskunergan@gmail.com>

extern "C" {
    fn wifi_connect();
}

pub struct Wifi {
    _private: (),
}

impl Wifi {
    pub fn wifi_connect() {
        unsafe { wifi_connect() };
    }
}
