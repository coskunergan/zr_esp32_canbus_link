// Copyright (c) 2025
// SPDX-License-Identifier: Apache-2.0
// Coskun ERGAN <coskunergan@gmail.com>

use crate::eeprom_int::EepromInt;
use core::str;
use heapless::String;
use zephyr::kconfig::{CONFIG_WIFI_SAMPLE_PSK as PSK_RAW, CONFIG_WIFI_SAMPLE_SSID as SSID_RAW};

extern "C" {
    fn wifi_connect();
}

fn get_default_ssid() -> String<32> {
    let mut s = String::<32>::new();
    let cleaned = SSID_RAW
        .as_bytes()
        .splitn(2, |&b| b == 0)
        .next()
        .unwrap_or(&[]);
    if let Ok(text) = str::from_utf8(cleaned) {
        let _ = s.push_str(text);
    }
    if s.is_empty() {
        let _ = s.push_str("MyWiFi");
    }
    s
}

fn get_default_psk() -> String<64> {
    let mut s = String::<64>::new();
    let cleaned = PSK_RAW
        .as_bytes()
        .splitn(2, |&b| b == 0)
        .next()
        .unwrap_or(&[]);
    if let Ok(text) = str::from_utf8(cleaned) {
        let _ = s.push_str(text);
    }
    if s.is_empty() {
        let _ = s.push_str("12345678");
    }
    s
}

pub struct Wifi {
    _private: (),
}

impl Wifi {
    pub fn wifi_connect() {
        let eeprom = EepromInt::new();

        let (ssid, psk) = match read_wifi_credentials(&eeprom) {
            Some((s, p)) if !s.is_empty() && !p.is_empty() => {
                log::info!("EEPROM'dan WiFi okundu: SSID=\"{}\"", s);
                (s, p)
            }
            _ => {
                log::info!("EEPROM boş → Kconfig varsayılanı kullanılıyor");
                (get_default_ssid(), get_default_psk())
            }
        };
        unsafe {
            set_wifi_credentials(ssid.as_bytes(), psk.as_bytes());
        }
        unsafe { wifi_connect() };
    }
}

type Ssid = String<32>;
type Psk = String<64>;

fn read_wifi_credentials(eeprom: &EepromInt) -> Option<(Ssid, Psk)> {
    let buf: [u8; 98] = eeprom.read(0).ok()?;

    let ssid_len = buf[0] as usize;
    let psk_len = buf[33] as usize;

    if ssid_len == 0 || ssid_len > 32 || psk_len == 0 || psk_len > 64 {
        return None;
    }

    let ssid_slice = &buf[1..1 + ssid_len];
    let psk_slice = &buf[34..34 + psk_len];

    let ssid_str = str::from_utf8(ssid_slice).ok()?;
    let psk_str = str::from_utf8(psk_slice).ok()?;

    let mut ssid = Ssid::new();
    let mut psk = Psk::new();

    let _ = ssid.push_str(ssid_str);
    let _ = psk.push_str(psk_str);

    Some((ssid, psk))
}

pub fn write_wifi_credentials(eeprom: &EepromInt, ssid: &Ssid, psk: &Psk) -> Result<(), ()> {
    let mut buf = [0u8; 98];

    let ssid_b = ssid.as_bytes();
    let psk_b = psk.as_bytes();

    if ssid_b.len() > 32 || psk_b.len() > 64 {
        return Err(());
    }

    buf[0] = ssid_b.len() as u8;
    buf[1..1 + ssid_b.len()].copy_from_slice(ssid_b);

    buf[33] = psk_b.len() as u8;
    buf[34..34 + psk_b.len()].copy_from_slice(psk_b);

    eeprom.write(0, &buf).map_err(|_| ())
}

static mut CURRENT_SSID: [u8; 33] = [0; 33];
static mut CURRENT_PSK: [u8; 65] = [0; 65];

unsafe fn set_wifi_credentials(ssid: &[u8], psk: &[u8]) {
    CURRENT_SSID = [0; 33];
    CURRENT_PSK = [0; 65];

    let ssid_len = ssid.len().min(32);
    CURRENT_SSID[..ssid_len].copy_from_slice(&ssid[..ssid_len]);

    let psk_len = psk.len().min(64);
    CURRENT_PSK[..psk_len].copy_from_slice(&psk[..psk_len]);
}

#[no_mangle]
pub unsafe extern "C" fn get_current_ssid() -> *const u8 {
    CURRENT_SSID.as_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn get_current_ssid_len() -> u8 {
    CURRENT_SSID.iter().position(|&b| b == 0).unwrap_or(32) as u8
}

#[no_mangle]
pub unsafe extern "C" fn get_current_psk() -> *const u8 {
    CURRENT_PSK.as_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn get_current_psk_len() -> u8 {
    CURRENT_PSK.iter().position(|&b| b == 0).unwrap_or(64) as u8
}
