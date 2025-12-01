// Copyright (c) 2025
// SPDX-License-Identifier: Apache-2.0
// Coskun ERGAN <coskunergan@gmail.com>

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};

extern "C" {
    fn mongoose_init();
    fn mongoose_poll();
    fn glue_set_temperature(new_temp: i32);
    fn glue_set_humidity(new_humi: i32);
    fn glue_get_leds(data: *mut Leds);
    fn glue_set_leds(data: *const Leds);
    fn glue_get_network_settings(data: *mut NetworkSettings);
    fn glue_set_network_settings(data: *const NetworkSettings);
    fn glue_get_settings(data: *mut Settings);
    fn glue_set_settings(data: *const Settings);
    fn glue_get_security(data: *mut Security);
    fn glue_set_security(data: *const Security);
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Leds {
    pub led1: bool,
    pub led2: bool,
    pub led3: bool,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub struct NetworkSettings {
    pub ip_address: [u8; 20],
    pub gw_address: [u8; 20],
    pub netmask: [u8; 20],
    pub dhcp: bool,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub struct Settings {
    pub string_val: [u8; 40],
    pub log_level: i32,
    pub double_val: f64,
    pub int_val: i32,
    pub bool_val: bool,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub struct Security {
    pub admin_password: [u8; 40],
    pub user_password: [u8; 40],
}

#[derive(Clone, Copy)]
pub enum MongooseEvent {
    LedChanged(Leds),
    NetworkSettingsChanged(NetworkSettings),
    SettingsChanged(Settings),
    SecurityChanged(Security),
}

pub static MG_EVENT_SIGNAL: Signal<CriticalSectionRawMutex, MongooseEvent> = Signal::new();
static mut MONGOOSE_INITED: bool = false;

#[derive(Clone, Copy)]
pub struct Mongoose {
    _private: (),
}

impl Mongoose {
    pub fn new(spawner: Spawner) -> Self {
        unsafe {
            mongoose_init();

            // ←←← BURASI ARTIK TAMAMEN GÜVENLİ unsafe BLOK İÇİNDE ←←←
            if !MONGOOSE_INITED {
                MONGOOSE_INITED = true;

                // mongoose_task sadece bir kere spawn olur
                spawner.must_spawn(mongoose_task());

                // Memory ordering (çok önemli, yarış koşullarını tamamen engeller)
                core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            }
        }

        Self { _private: () }
    }

    pub fn mg_poll(&self) {
        unsafe { mongoose_poll() };
    }
    #[allow(dead_code)]
    pub fn set_temperature(&self, temp: i32) {
        unsafe {
            glue_set_temperature(temp);
        }
    }
    #[allow(dead_code)]
    pub fn set_humidity(&self, temp: i32) {
        unsafe {
            glue_set_humidity(temp);
        }
    }
    #[allow(dead_code)]
    pub fn get_leds(&self, led: *mut Leds) {
        unsafe {
            glue_get_leds(led);
        }
    }
    #[allow(dead_code)]
    pub fn set_leds(&self, led: *const Leds) {
        unsafe {
            glue_set_leds(led);
        }
    }
    #[allow(dead_code)]
    pub fn get_network_settings(&self, settings: *mut NetworkSettings) {
        unsafe {
            glue_get_network_settings(settings);
        }
    }
    #[allow(dead_code)]
    pub fn set_network_settings(&self, settings: *const NetworkSettings) {
        unsafe {
            glue_set_network_settings(settings);
        }
    }
    #[allow(dead_code)]
    pub fn get_settings(&self, settings: *mut Settings) {
        unsafe {
            glue_get_settings(settings);
        }
    }
    #[allow(dead_code)]
    pub fn set_settings(&self, settings: *const Settings) {
        unsafe {
            glue_set_settings(settings);
        }
    }
    #[allow(dead_code)]
    pub fn get_security(&self, security: *mut Security) {
        unsafe {
            glue_get_security(security);
        }
    }
    #[allow(dead_code)]
    pub fn set_security(&self, security: *const Security) {
        unsafe {
            glue_set_security(security);
        }
    }

    pub async fn wait_event(&self) -> MongooseEvent {
        MG_EVENT_SIGNAL.wait().await
    }
    #[allow(dead_code)]
    pub fn try_get_event(&self) -> Option<MongooseEvent> {
        MG_EVENT_SIGNAL.try_take()
    }
}

#[embassy_executor::task]
async fn mongoose_task() {
    loop {
        mg_event_check();
        Timer::after(Duration::from_millis(100)).await;
    }
}

static mut CURR_LEDS: Leds = Leds {
    led1: false,
    led2: false,
    led3: false,
};

static mut CURR_NETWORK: NetworkSettings = NetworkSettings {
    ip_address: [0; 20],
    gw_address: [0; 20],
    netmask: [0; 20],
    dhcp: false,
};

static mut CURR_SETTINGS: Settings = Settings {
    string_val: [0; 40],
    log_level: 0,
    double_val: 0.0,
    int_val: 0,
    bool_val: false,
};

static mut CURR_SECURITY: Security = Security {
    admin_password: [0; 40],
    user_password: [0; 40],
};

fn mg_event_check() {
    let mut leds = Leds {
        led1: false,
        led2: false,
        led3: false,
    };

    unsafe {
        glue_get_leds(&mut leds as *mut Leds);
        if leds != CURR_LEDS {
            CURR_LEDS = leds;
            MG_EVENT_SIGNAL.signal(MongooseEvent::LedChanged(leds));
        }
    }

    let mut network = NetworkSettings {
        ip_address: [0; 20],
        gw_address: [0; 20],
        netmask: [0; 20],
        dhcp: false,
    };
    unsafe {
        glue_get_network_settings(&mut network);
        if network != CURR_NETWORK {
            CURR_NETWORK = network;
            MG_EVENT_SIGNAL.signal(MongooseEvent::NetworkSettingsChanged(network));
        }
    }

    let mut settings = Settings {
        string_val: [0; 40],
        log_level: 0,
        double_val: 0.0,
        int_val: 0,
        bool_val: false,
    };
    unsafe {
        glue_get_settings(&mut settings);
        if settings != CURR_SETTINGS {
            CURR_SETTINGS = settings;
            MG_EVENT_SIGNAL.signal(MongooseEvent::SettingsChanged(settings));
        }
    }
    let mut security = Security {
        admin_password: [0; 40],
        user_password: [0; 40],
    };
    unsafe {
        glue_get_security(&mut security);
        if security != CURR_SECURITY {
            CURR_SECURITY = security;
            MG_EVENT_SIGNAL.signal(MongooseEvent::SecurityChanged(security));
        }
    }
}
