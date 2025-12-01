// Copyright (c) 2025
// SPDX-License-Identifier: Apache-2.0
// Coskun ERGAN <coskunergan@gmail.com>

#![no_std]

extern crate alloc;
use alloc::format;

use embassy_time::{Duration, Timer};

#[cfg(feature = "executor-thread")]
use embassy_executor::Executor;

#[cfg(feature = "executor-zephyr")]
use zephyr::embassy::Executor;

use embassy_executor::Spawner;
use static_cell::StaticCell;

use zephyr::device::gpio::GpioPin;

use portable_atomic::{AtomicF32, AtomicU16, Ordering};
//use core::{sync::atomic::AtomicU16, sync::atomic::Ordering};

use crate::mg::{Mongoose, MongooseEvent};
use canbus::CanBus;
use display_io::Display;
use modbus_slave::ModbusSlave;
use pin::{GlobalPin, Pin};
use sht21::Sht21;
use wifi::Wifi;

mod button;
mod canbus;
mod display_io;
mod mg;
mod modbus_slave;
mod pin;
mod sht21;
mod usage;
mod wifi;

static EXECUTOR_MAIN: StaticCell<Executor> = StaticCell::new();
static RED_LED_PIN: GlobalPin = GlobalPin::new();
static GREEN_LED_PIN: GlobalPin = GlobalPin::new();
static BLUE_LED_PIN: GlobalPin = GlobalPin::new();

static COUNTER: AtomicU16 = AtomicU16::new(0);
static REGISTER: AtomicU16 = AtomicU16::new(0);

static TEMP_VAL: AtomicF32 = AtomicF32::new(0.0);
static HUMI_VAL: AtomicF32 = AtomicF32::new(0.0);

const VERSION_MAJOR: &str = env!("VERSION_MAJOR");
const VERSION_MINOR: &str = env!("VERSION_MINOR");
const PATCHLEVEL: &str = env!("PATCHLEVEL");
const EXTRAVERSION: &str = env!("EXTRAVERSION");

//====================================================================================
//====================================================================================
#[embassy_executor::task]
async fn led_task(spawner: Spawner) {
    let button = zephyr::devicetree::labels::button::get_instance().unwrap();

    declare_buttons!(
        spawner,
        [(
            button,
            || {
                zephyr::printk!("Button Pressed!\n");
                REGISTER.fetch_add(1, Ordering::SeqCst);
            },
            Duration::from_millis(10)
        )]
    );

    let sensor = Sht21::new().expect("SHT21 not start.");

    let display = Display::new();
    display.set_backlight(1);

    loop {
        match sensor.read_data() {
            Ok((temp, hum)) => {
                log::info!("Temperature: {:.2}°C | Humidity: {:.2}%RH", temp, hum);
                TEMP_VAL.store(temp, Ordering::Relaxed);
                HUMI_VAL.store(hum, Ordering::Relaxed);
                display.clear();
                let msg = format!("Tempera: {:2.2} CHumidity: {:2.1} %", temp, hum);
                display.write(msg.as_bytes());
            }
            Err(e) => {
                log::info!("Read error! : {:?}", e);
                display.clear();
                let msg = format!("Read Error!");
                display.write(msg.as_bytes());
            }
        }

        COUNTER.fetch_add(1, Ordering::SeqCst);
        log::info!(
            "Loop! Version: {}.{}.{} ({})",
            VERSION_MAJOR,
            VERSION_MINOR,
            PATCHLEVEL,
            EXTRAVERSION
        );
        let _ = Timer::after(Duration::from_millis(1000)).await;
    }
}
//====================================================================================
#[embassy_executor::task]
async fn canbus_task(can: CanBus) {
    loop {
        let message = format!("BTN:{}", REGISTER.load(Ordering::SeqCst));
        let _ = can.canbus_isotp_send(message.as_bytes());
        Timer::after(Duration::from_millis(1000)).await;
    }
}
//====================================================================================
#[embassy_executor::task]
async fn mg_event_task(mg: Mongoose) {
    let red_led_pin = RED_LED_PIN.get();
    let green_led_pin = GREEN_LED_PIN.get();
    let blue_led_pin = BLUE_LED_PIN.get();
    loop {
        let event = mg.wait_event().await;
        match event {
            MongooseEvent::LedChanged(leds) => {
                red_led_pin.set(leds.led1);
                green_led_pin.set(leds.led2);
                blue_led_pin.set(leds.led3);
                log::warn!("LEDler değişti");
            }
            MongooseEvent::NetworkSettingsChanged(_net) => {
                log::warn!("Network ayarları değişti.");
            }
            MongooseEvent::SettingsChanged(_s) => {
                log::warn!("Settings değişti");
            }
            MongooseEvent::SecurityChanged(security) => {
                let admin = core::str::from_utf8(
                    &security
                        .admin_password
                        .split(|&b| b == 0)
                        .next()
                        .unwrap_or(&[]),
                )
                .unwrap_or("?");
                let user = core::str::from_utf8(
                    &security
                        .user_password
                        .split(|&b| b == 0)
                        .next()
                        .unwrap_or(&[]),
                )
                .unwrap_or("?");
                log::warn!("Security güncellendi! Admin: {}, User: {}", admin, user);
            }
        }
    }
}
//====================================================================================
#[embassy_executor::task]
async fn mg_task(spawner: Spawner) {
    Timer::after(Duration::from_millis(1000)).await;
    let mg = Mongoose::new(spawner);
    spawner.spawn(mg_event_task(mg)).unwrap();

    loop {
        Timer::after(Duration::from_millis(1)).await;
        mg.mg_poll();
        mg.set_temperature(TEMP_VAL.load(Ordering::Relaxed) as i32);
        mg.set_humidity(HUMI_VAL.load(Ordering::Relaxed) as i32);
    }
}
//====================================================================================
fn receive_callback(data: &[u8]) {
    if let Ok(s) = core::str::from_utf8(data) {
        log::info!("Received data ({} byte): {}", data.len(), s);
    } else {
        log::info!(
            "Received data is not a valid UTF-8 string. Raw data ({} bytes): {:?}",
            data.len(),
            data
        );
    }
}
//====================================================================================
#[no_mangle]
extern "C" fn rust_main() {
    let _ = usage::set_logger();

    log::info!("Restart!!!\r\n");

    RED_LED_PIN.init(Pin::new(
        zephyr::devicetree::labels::red_led::get_instance().expect("my_red_led not found!"),
    ));
    GREEN_LED_PIN.init(Pin::new(
        zephyr::devicetree::labels::green_led::get_instance().expect("my_green_led not found!"),
    ));
    BLUE_LED_PIN.init(Pin::new(
        zephyr::devicetree::labels::blue_led::get_instance().expect("my_blue_led not found!"),
    ));

    Wifi::wifi_connect();

    let mut local_reg = 0x123;

    let mut canbus = CanBus::new("canbus0\0");
    canbus.set_data_callback(receive_callback);

    // let modbus_vcp = ModbusSlave::new("modbus0\0");
    let modbus = ModbusSlave::new("modbus1\0");

    modbus.mb_add_holding_reg(COUNTER.as_ptr(), 0);
    modbus.mb_add_holding_reg(REGISTER.as_ptr(), 1);
    modbus.mb_add_holding_reg(&mut local_reg, 2);

    // modbus_vcp.mb_add_holding_reg(COUNTER.as_ptr(), 0);
    // modbus_vcp.mb_add_holding_reg(REGISTER.as_ptr(), 1);
    // modbus_vcp.mb_add_holding_reg(&mut local_reg, 2);

    let executor = EXECUTOR_MAIN.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(led_task(spawner)).unwrap();
        spawner.spawn(mg_task(spawner)).unwrap();
        spawner.spawn(canbus_task(canbus)).unwrap();
    })
}
//====================================================================================
//====================================================================================
//====================================================================================
