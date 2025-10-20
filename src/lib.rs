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

use portable_atomic::{AtomicU16, Ordering};
//use core::{sync::atomic::AtomicU16, sync::atomic::Ordering};

// use canbus::CanBus;
use display_io::Display;
use modbus_slave::ModbusSlave;
use pin::{GlobalPin, Pin};

mod button;
// mod canbus;
mod display_io;
mod modbus_slave;
mod pin;
mod usage;

static EXECUTOR_MAIN: StaticCell<Executor> = StaticCell::new();
static RED_LED_PIN: GlobalPin = GlobalPin::new();

static COUNTER: AtomicU16 = AtomicU16::new(0);
static REGISTER: AtomicU16 = AtomicU16::new(0);

//====================================================================================
//====================================================================================
#[embassy_executor::task]
async fn led_task(spawner: Spawner) {
    let red_led_pin = RED_LED_PIN.get();

    let button = zephyr::devicetree::labels::button::get_instance().unwrap();

    declare_buttons!(
        spawner,
        [(
            button,
            || {
                zephyr::printk!("Button Pressed!\n");
                REGISTER.fetch_add(1, Ordering::SeqCst);
                red_led_pin.toggle();
            },
            Duration::from_millis(10)
        )]
    );

    let display = Display::new();
    display.set_backlight(1);

    loop {
        display.clear();
        let msg = format!(
            "REG: {:5}      Coskun Ergan!",
            REGISTER.load(Ordering::SeqCst)
        );
        display.write(msg.as_bytes());
        red_led_pin.toggle();
        COUNTER.fetch_add(1, Ordering::SeqCst);
        log::info!("Endless Loop!");
        let _ = Timer::after(Duration::from_millis(500)).await;
    }
}
//====================================================================================
// #[embassy_executor::task]
// async fn canbus_task(can: CanBus) {
//     loop {
//         let message = format!(
//             "BUTTON PRESS:{} COUNTER: {} ",
//             REGISTER.load(Ordering::SeqCst),
//             COUNTER.load(Ordering::SeqCst)
//         );
//         let _ = can.canbus_isotp_send(message.as_bytes());
//         Timer::after(Duration::from_millis(100)).await;
//     }
// }
// //====================================================================================
// fn receive_callback(data: &[u8]) {
//     if let Ok(s) = core::str::from_utf8(data) {
//         log::info!("Received data ({} byte): {}", data.len(), s);
//     } else {
//         log::info!(
//             "Received data is not a valid UTF-8 string. Raw data ({} bytes): {:?}",
//             data.len(),
//             data
//         );
//     }
// }
//====================================================================================
#[no_mangle]
extern "C" fn rust_main() {
    let _ = usage::set_logger();

    log::info!("Restart!!!\r\n");

    RED_LED_PIN.init(Pin::new(
        zephyr::devicetree::labels::red_led::get_instance().expect("my_red_led not found!"),
    ));

    let mut local_reg = 0x123;

    // let mut canbus = CanBus::new("canbus0\0");
    // canbus.set_data_callback(receive_callback);

    let modbus_vcp = ModbusSlave::new("modbus0\0");
    let modbus = ModbusSlave::new("modbus1\0");

    modbus.mb_add_holding_reg(COUNTER.as_ptr(), 0);
    modbus.mb_add_holding_reg(REGISTER.as_ptr(), 1);
    modbus.mb_add_holding_reg(&mut local_reg, 2);

    modbus_vcp.mb_add_holding_reg(COUNTER.as_ptr(), 0);
    modbus_vcp.mb_add_holding_reg(REGISTER.as_ptr(), 1);
    modbus_vcp.mb_add_holding_reg(&mut local_reg, 2);    

    let executor = EXECUTOR_MAIN.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(led_task(spawner)).unwrap();
        // spawner.spawn(canbus_task(canbus)).unwrap();
    })
}
//====================================================================================
//====================================================================================
//====================================================================================
