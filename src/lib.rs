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
use zephyr::raw;

use portable_atomic::{AtomicU16, Ordering};
//use core::{sync::atomic::AtomicU16, sync::atomic::Ordering};

use canbus::CanBus;
use display_io::Display;
use mg::Mongoose;
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

const VERSION_MAJOR: &str = env!("VERSION_MAJOR");
const VERSION_MINOR: &str = env!("VERSION_MINOR");
const PATCHLEVEL: &str = env!("PATCHLEVEL");
const EXTRAVERSION: &str = env!("EXTRAVERSION");

#[repr(C)]
pub struct Leds {
    pub led1: bool,
    pub led2: bool,
    pub led3: bool,
}

extern "C" {
    fn glue_set_temperature(new_temp: i32);
    fn glue_set_humidity(new_humi: i32);
    fn glue_get_leds(data: *mut Leds);
    fn stm32_flashing_start() -> i32;
}

pub fn set_temperature(temp: i32) {
    unsafe {
        glue_set_temperature(temp);
    }
}

pub fn set_humidity(temp: i32) {
    unsafe {
        glue_set_humidity(temp);
    }
}

pub fn get_leds(led: *mut Leds) {
    unsafe {
        glue_get_leds(led);
    }
}

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
                set_temperature(temp as i32);
                set_humidity(hum as i32);
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

        if REGISTER.load(Ordering::SeqCst) % 2 == 0 {
            REGISTER.fetch_add(1, Ordering::SeqCst);
            unsafe {
                stm32_flashing_start();
            }
        }
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
async fn mg_task() {
    Timer::after(Duration::from_millis(1000)).await;
    let mg = Mongoose::new();
    let red_led_pin = RED_LED_PIN.get();
    let green_led_pin = GREEN_LED_PIN.get();
    let blue_led_pin = BLUE_LED_PIN.get();
    let mut state = Leds {
        led1: false,
        led2: false,
        led3: false,
    };
    loop {
        Timer::after(Duration::from_millis(1)).await;
        mg.mg_poll();
        get_leds(&mut state as *mut Leds);
        red_led_pin.set(state.led1);
        green_led_pin.set(state.led2);
        blue_led_pin.set(state.led3);
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

    unsafe {
        raw::k_thread_priority_set(raw::k_current_get(), 5);
    }

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
    // let modbus = ModbusSlave::new("modbus1\0");

    // modbus.mb_add_holding_reg(COUNTER.as_ptr(), 0);
    // modbus.mb_add_holding_reg(REGISTER.as_ptr(), 1);
    // modbus.mb_add_holding_reg(&mut local_reg, 2);

    // modbus_vcp.mb_add_holding_reg(COUNTER.as_ptr(), 0);
    // modbus_vcp.mb_add_holding_reg(REGISTER.as_ptr(), 1);
    // modbus_vcp.mb_add_holding_reg(&mut local_reg, 2);

    let executor = EXECUTOR_MAIN.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(led_task(spawner)).unwrap();
        spawner.spawn(mg_task()).unwrap();
        //spawner.spawn(canbus_task(canbus)).unwrap();
    })
}
//====================================================================================
//====================================================================================
//====================================================================================
