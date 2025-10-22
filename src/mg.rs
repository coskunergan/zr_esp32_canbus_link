// Copyright (c) 2025
// SPDX-License-Identifier: Apache-2.0
// Coskun ERGAN <coskunergan@gmail.com>

extern "C" {
    fn mg_init() -> i32;    
}

pub struct Mongoose {
    _private: (),
}

impl Mongoose {
    pub fn new() -> Self {
        let ret = unsafe { mg_init() };
        if ret != 0 {
            panic!("Failed to initialize Mongoose: error {}", ret);
        }
        Mongoose { _private: () }
    }    
}
