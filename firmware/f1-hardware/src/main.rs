#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]


mod hd108;
mod driver_info;
use crate::driver_info::DRIVERS;
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
use static_cell::StaticCell;

#[derive(Debug, bincode::Encode, bincode::Decode, PartialEq, Default)]
pub struct DriverData {
    pub driver_number: u8,
    pub led_num: u8,
}

pub const NUM_DRIVERS: usize = 20;

#[derive(Debug, bincode::Encode, bincode::Decode, PartialEq, Default)]
pub struct UpdateFrame {
    pub frame: [DriverData; NUM_DRIVERS],
}

impl UpdateFrame {
    pub const SERIALIZED_SIZE: usize = NUM_DRIVERS * 2;

    pub fn to_bytes(&self) -> Result<[u8; Self::SERIALIZED_SIZE], ()> {
        let mut buf = [0u8; Self::SERIALIZED_SIZE];
        let config = bincode::config::standard()
            .with_little_endian()
            .with_fixed_int_encoding();
        if let Ok(l) = bincode::encode_into_slice(&self, &mut buf[..], config) {
            if l == Self::SERIALIZED_SIZE {
                return Ok(buf);
            }
        }
        Err(())
    }

    pub fn try_from_bytes(buf: &[u8]) -> Result<Self, ()> {
        let config = bincode::config::standard()
            .with_little_endian()
            .with_fixed_int_encoding();
        if let Ok((frame, _n)) = bincode::decode_from_slice(buf, config) {
            Ok(frame)
        } else {
            Err(())
        }
    }
}



enum Message {
    ButtonPressed,
}

static SIGNAL_CHANNEL: StaticCell<Channel<NoopRawMutex, Message, 1>> = StaticCell::new();

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

    let signal_channel = SIGNAL_CHANNEL.init(Channel::new());

    // Spawn the button task with ownership of the button pin and the sender
    spawner
        .spawn(button_task(button_pin, signal_channel.sender()))
        .unwrap();

    // Spawn the led task with the receiver
    spawner
        .spawn(led_task(hd108, signal_channel.receiver()))
        .unwrap();
}

#[embassy_executor::task]
async fn led_task(
    mut hd108: HD108<impl SpiBus<u8> + 'static>,
    receiver: Receiver<'static, NoopRawMutex, Message, 1>,
) {
    loop {
        // Wait for the start message
        receiver.receive().await;

        println!("Button pressed, starting race...");

        // Start deserialization in chunks
        let data_bin = include_bytes!("output.bin");
        let mut remaining_data = &data_bin[..];

        while !remaining_data.is_empty() {
            // Attempt to deserialize a single frame from the data using `try_from_bytes`
            match UpdateFrame::try_from_bytes(remaining_data) {
                Ok(frame) => {
                    // Move the remaining_data pointer forward by the size of the serialized frame
                    let frame_size = UpdateFrame::SERIALIZED_SIZE;
                    remaining_data = &remaining_data[frame_size..];

                    // Prepare LED updates
                    let mut led_updates: heapless08::Vec<(usize, u8, u8, u8), 20> = heapless08::Vec::new();
                    for driver_data in &frame.frame {
                        if let Some(driver) = DRIVERS.iter().find(|d| d.number == driver_data.driver_number as u32) {
                            led_updates.push((
                                driver_data.led_num as usize,
                                driver.color.0,
                                driver.color.1,
                                driver.color.2,
                            )).unwrap();
                        }
                    }

                    // Set the LEDs for this frame
                    if let Err(err) = hd108.set_leds(&led_updates).await {
                        println!("Failed to set LEDs: {:?}", err);
                    }

                    // Wait for the next frame update
                    Timer::after(Duration::from_millis(50)).await;
                }
                Err(_) => {
                    println!("Failed to deserialize frame");
                    break;
                }
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

#[embassy_executor::task]
async fn button_task(
    mut button_pin: Input<'static, GpioPin<10>>,
    sender: Sender<'static, NoopRawMutex, Message, 1>,
) {
    loop {
        // Wait for a button press
        button_pin.wait_for_falling_edge().await;
        sender.send(Message::ButtonPressed).await;
        Timer::after(Duration::from_millis(400)).await; // Debounce delay
    }
}
