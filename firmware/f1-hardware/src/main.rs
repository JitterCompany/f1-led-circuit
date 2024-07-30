#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod driver_info;
mod hd108;
use driver_info::{DriverInfo, DRIVERS};

use core::alloc::GlobalAlloc;
use core::alloc::Layout;
use core::fmt;
use core::fmt::Debug;
use core::marker::Sized;
use core::mem::MaybeUninit;
use core::option::Option;
use core::ptr::null_mut;
use core::result::Result;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::channel::Receiver;
use embassy_sync::channel::Sender;
use embassy_time::{Duration, Timer};
use embedded_hal_async::spi::SpiBus;
use esp_backtrace as _;
use esp_hal::dma::DmaDescriptor;
use esp_hal::spi::master::prelude::_esp_hal_spi_master_dma_WithDmaSpi2;
use esp_hal::{
    clock::ClockControl,
    dma::{Dma, DmaPriority},
    gpio::{Event, GpioPin, Input, Io, Pull},
    peripherals::Peripherals,
    prelude::*,
    spi::{master::Spi, SpiMode},
    system::SystemControl,
    timer::timg::TimerGroup,
};
use esp_println::println;
use hd108::HD108;
use heapless08::Vec;
use panic_halt as _;
use serde::{Deserialize, Serialize};
use static_cell::StaticCell;
//use grounded::uninit::GroundedCell;
//use postcard::from_bytes;
//use serde::de::{self, Deserializer, SeqAccess, Visitor};
//use serde::ser::{SerializeSeq, Serializer};
//use serde_json_core::de::from_slice;

#[derive(Serialize, Clone, Copy, Deserialize, PartialEq, Debug)]
pub struct DriverData {
    pub driver_number: u8,
    pub led_num: u8,
}

#[derive(Serialize, Clone, Copy, Deserialize, PartialEq, Debug)]
pub struct UpdateFrame {
    pub frame: [Option<DriverData>; 20],
}

#[derive(PartialEq, Debug)]
pub struct VisualizationData {
    pub update_rate_ms: u32,
    pub frames: heapless08::Vec<UpdateFrame, 8879>,
}

enum ButtonMessage {
    ButtonPressed,
}

static BUTTON_CHANNEL: StaticCell<Channel<NoopRawMutex, ButtonMessage, 1>> = StaticCell::new();

#[main]
async fn main(spawner: Spawner) {
    println!("Starting program!...");

    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    let timg0 = TimerGroup::new_async(peripherals.TIMG0, &clocks);
    esp_hal_embassy::init(&clocks, timg0);

    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);

    let sclk = io.pins.gpio6;
    let miso = io.pins.gpio8;
    let mosi = io.pins.gpio7;
    let cs = io.pins.gpio9;

    let dma = Dma::new(peripherals.DMA);

    let dma_channel = dma.channel0;

    static TX_DESC: StaticCell<[DmaDescriptor; 8]> = StaticCell::new();
    let tx_descriptors = TX_DESC.init([DmaDescriptor::EMPTY; 8]);

    static RX_DESC: StaticCell<[DmaDescriptor; 8]> = StaticCell::new();
    let rx_descriptors = RX_DESC.init([DmaDescriptor::EMPTY; 8]);

    let spi = Spi::new(peripherals.SPI2, 20.MHz(), SpiMode::Mode0, &clocks)
        .with_pins(Some(sclk), Some(mosi), Some(miso), Some(cs))
        .with_dma(dma_channel.configure_for_async(
            false,
            tx_descriptors,
            rx_descriptors,
            DmaPriority::Priority0,
        ));

    let hd108 = HD108::new(spi);

    // Initialize the button pin as input with interrupt and pull-up resistor
    let mut button_pin = Input::new(io.pins.gpio10, Pull::Up);

    // Enable interrupts for the button pin
    button_pin.listen(Event::FallingEdge);

    let button_channel = BUTTON_CHANNEL.init(Channel::new());

    // Spawn the button task with ownership of the button pin and the sender
    spawner
        .spawn(button_task(button_pin, button_channel.sender()))
        .unwrap();

    // Spawn the run race task with the receiver
    spawner
        .spawn(led_task(hd108, button_channel.receiver()))
        .unwrap();
}

#[embassy_executor::task]
async fn led_task(
    mut hd108: HD108<impl SpiBus<u8> + 'static>,
    receiver: Receiver<'static, NoopRawMutex, ButtonMessage, 1>,
) {
    loop {
        // Wait for the button press signal
        receiver.receive().await;

        println!("Button pressed, starting race...");

        // Start deserialization in chunks
        let data_bin = include_bytes!("data.bin");
        let mut remaining_data = &data_bin[..];

        while remaining_data.len() >= 40 {
            let mut frame = UpdateFrame { frame: [None; 20] };

            for i in 0..20 {
                // Deserialize the two bytes into DriverData
                let driver_number = remaining_data[2 * i];
                let led_num = remaining_data[2 * i + 1];
                frame.frame[i] = Some(DriverData {
                    driver_number,
                    led_num,
                });
            }

            remaining_data = &remaining_data[40..];

            // Prepare LED updates
            let mut led_updates: Vec<(usize, u8, u8, u8), 20> = Vec::new();
            for driver_data_option in &frame.frame {
                if let Some(driver_data) = driver_data_option {
                    if let Some(driver) = DRIVERS
                        .iter()
                        .find(|d| d.number == driver_data.driver_number as u32)
                    {
                        led_updates
                            .push((
                                driver_data.led_num as usize,
                                driver.color.0,
                                driver.color.1,
                                driver.color.2,
                            ))
                            .unwrap();
                    }
                }
            }

            // Set the LEDs for this frame
            if let Err(err) = hd108.set_leds(&led_updates).await {
                println!("Failed to set LEDs: {:?}", err);
            }

            // Wait for the next frame update
            Timer::after(Duration::from_millis(250)).await;
        }

        // Ensure LEDs are turned off at the end
        hd108.set_off().await.unwrap();
    }
}

#[embassy_executor::task]
async fn button_task(
    mut button_pin: Input<'static, GpioPin<10>>,
    sender: Sender<'static, NoopRawMutex, ButtonMessage, 1>,
) {
    loop {
        // Wait for a button press
        button_pin.wait_for_falling_edge().await;
        sender.send(ButtonMessage::ButtonPressed).await;
        Timer::after(Duration::from_millis(400)).await; // Debounce delay
    }
}

/*
// Test 1
#[embassy_executor::task]
async fn led_task(
    mut hd108: HD108<impl SpiBus<u8> + 'static>,
    receiver: Receiver<'static, NoopRawMutex, ButtonMessage, 1>,
) {
    loop {
        // Wait for the button press signal
        receiver.receive().await;

        println!("Button pressed, starting race...");

        // Start deserialization in chunks
        let data_bin = include_bytes!("data.bin");
        let mut remaining_data = &data_bin[..];

        while !remaining_data.is_empty() {
            // Attempt to deserialize a single frame from the data
            let result: Result<(UpdateFrame, &[u8]), postcard::Error> = postcard::take_from_bytes(remaining_data);

            match result {
                Ok((frame, rest)) => {
                    // Update remaining data to point to the rest
                    remaining_data = rest;

                    // Manually serialize the first frame to a simple format
                    println!("First frame data:");
                    for driver_data_option in &frame.frame {
                        if let Some(driver_data) = driver_data_option {
                            println!("Driver number: {}, LED number: {}", driver_data.driver_number, driver_data.led_num);
                        }
                    }

                    // Break after processing the first frame for debugging
                    break;
                }
                Err(err) => {
                    println!("Failed to deserialize frame: {:?}", err);
                    break;
                }
            }
        }

        // Ensure LEDs are turned off at the end
        hd108.set_off().await.unwrap();
    }
}
    */

/*
// Test 2

#[embassy_executor::task]
async fn led_task(
    mut hd108: HD108<impl SpiBus<u8> + 'static>,
    receiver: Receiver<'static, NoopRawMutex, ButtonMessage, 1>,
) {
    loop {
        // Wait for the button press signal
        receiver.receive().await;

        println!("Button pressed, starting race...");

        // Start deserialization in chunks
        let data_bin = include_bytes!("data.bin");
        let remaining_data = &data_bin[..];

        while !remaining_data.is_empty() {
            // Manually read the binary data
            let result: Result<(UpdateFrame, &[u8]), postcard::Error> = postcard::take_from_bytes(remaining_data);

            match result {
                Ok((frame, rest)) => {
                    // Update remaining data to point to the rest
                    //remaining_data = rest;

                    // Print the first frame in JSON format
                    if let Ok(json_str) = serde_json_core::ser::to_string::<_, 1024>(&frame) {
                        println!("First frame data: {}", json_str);
                    } else {
                        println!("Failed to serialize frame to JSON");
                    }

                    // Break after processing the first frame for debugging
                    break;
                }
                Err(err) => {
                    println!("Failed to deserialize frame: {:?}", err);
                    break;
                }
            }
        }
        // Ensure LEDs are turned off at the end
        hd108.set_off().await.unwrap();
    }
}
*/

/*
// Test 3

#[embassy_executor::task]
async fn led_task(
    mut hd108: HD108<impl SpiBus<u8> + 'static>,
    receiver: Receiver<'static, NoopRawMutex, ButtonMessage, 1>,
) {
    loop {
        // Wait for the button press signal
        receiver.receive().await;

        println!("Button pressed, starting race...");

        // Start deserialization in chunks
        let data_bin = include_bytes!("data.bin");
        let mut remaining_data: &[u8] = data_bin; // Use &[u8] instead of &[u8; 532748]

        while !remaining_data.is_empty() {
            // Attempt to interpret the data as JSON
            if let Ok(json_str) = core::str::from_utf8(remaining_data) {
                println!("Interpreting data as JSON string...");
                match from_slice::<UpdateFrame>(json_str.as_bytes()) {
                    Ok((frame, remaining)) => {
                        // Update remaining data
                        remaining_data = &remaining_data[remaining..];

                        // Prepare LED updates (not shown for brevity)
                        // ...

                        // Wait for the next frame update
                        Timer::after(Duration::from_millis(250)).await;
                    }
                    Err(err) => {
                        println!("Failed to deserialize frame: {:?}", err);
                        break;
                    }
                }
            } else {
                println!("Data is not valid UTF-8");
                break;
            }

            // Check if a stop message was received
            if receiver.try_receive().is_ok() {
                hd108.set_off().await.unwrap();
                break;
            }
        }

        // Ensure LEDs are turned off at the end
        hd108.set_off().await.unwrap();
    }
}
*/
