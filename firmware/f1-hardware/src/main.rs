#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod driver_info;
mod hd108;
use crate::driver_info::DRIVERS;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::channel::Receiver;
use embassy_sync::channel::Sender;
use embassy_time::{Duration, Timer};
use embedded_hal_async::spi::SpiBus;
use esp_backtrace as _;
use esp_hal::analog::adc::AdcPin;
use esp_hal::dma::DmaDescriptor;
use esp_hal::spi::master::prelude::_esp_hal_spi_master_dma_WithDmaSpi2;
use esp_hal::{
    analog::adc::{Adc, AdcConfig, Attenuation},
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
use f1_logic::data_frame::{DriverData, UpdateFrame, NUM_DRIVERS};
use hd108::HD108;
use heapless08::Vec;
use panic_halt as _;
use static_cell::StaticCell;

enum Message {
    ButtonPressed,
}

static SIGNAL_CHANNEL: StaticCell<Channel<NoopRawMutex, Message, 1>> = StaticCell::new();

type AdcCal = esp_hal::analog::adc::AdcCalLine<esp_hal::peripherals::ADC1>;

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

#[embassy_executor::task]
async fn temperature_task(
    mut adc1: Adc<'static, esp_hal::peripherals::ADC1>,
    mut adc1_pin: AdcPin<GpioPin<1>, esp_hal::peripherals::ADC1, AdcCal>,
) {
    loop {
        // Non-blocking read of ADC value
        let mut pin_mv = None;
        loop {
            match adc1.read_oneshot(&mut adc1_pin) {
                Ok(value) => {
                    pin_mv = Some(value);
                    break;
                }
                Err(nb::Error::WouldBlock) => {
                    // ADC is not ready, wait for a short duration to avoid busy-waiting
                    Timer::after(Duration::from_millis(10)).await;
                }
                Err(e) => {
                    // Handle other errors if necessary
                    println!("ADC read error: {:?}", e);
                    break;
                }
            }
        }

        if let Some(pin_mv) = pin_mv {
            // Convert to temperature
            let temperature_c = convert_voltage_to_temperature(pin_mv);
            // Print temperature
            println!("Temperature: {:.2} °C", temperature_c);
        }

        // Wait for 1 second before the next reading
        Timer::after(Duration::from_secs(1)).await;
    }
}

#[embassy_executor::task]
async fn led_task(
    mut hd108: HD108<impl SpiBus<u8> + 'static>,
    receiver: Receiver<'static, NoopRawMutex, Message, 1>,
) {
    // Define the brightness levels
    let low_brightness = 10; // Low brightness for background LEDs

    // Start the train animation immediately
    let high_brightness = 255;
    let led_count = 97;
    let train_length = 15;
    let colors = [
        (high_brightness, 0, 0),
        (high_brightness, 0, 0),
        (high_brightness, 0, 0),
        (high_brightness, 0, 0),
        (high_brightness, 0, 0),
        (0, 0, high_brightness),
        (0, 0, high_brightness),
        (0, 0, high_brightness),
        (0, 0, high_brightness),
        (0, 0, high_brightness),
        (0, high_brightness, 0),
        (0, high_brightness, 0),
        (0, high_brightness, 0),
        (0, high_brightness, 0),
        (0, high_brightness, 0),
    ];

    let mut iteration_count = 0;

    while iteration_count < 10 {
        for i in 0..led_count {
            let mut led_updates: heapless08::Vec<(usize, u8, u8, u8), 97> = heapless08::Vec::new();

            // Set all LEDs to low brightness
            for j in 0..led_count {
                led_updates
                    .push((j, low_brightness, low_brightness, low_brightness))
                    .unwrap();
            }

            // Update the train LEDs with high brightness colors
            for j in 0..train_length {
                let pos = (i + j) % led_count;
                let color = colors[j];
                led_updates[pos] = (pos, color.0, color.1, color.2);
            }

            hd108.set_leds(&led_updates).await.unwrap();
            Timer::after(Duration::from_millis(10)).await;
        }
        iteration_count += 1;
    }

    println!("Startup animation complete...");

    // Set all leds off
    hd108.set_off().await.unwrap();

    loop {
        // Wait for the start message
        receiver.receive().await;

        println!("Starting race...");

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
                    let mut led_updates: heapless08::Vec<(usize, u8, u8, u8), 20> =
                        heapless08::Vec::new();
                    for driver_data in &frame.frame {
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

fn convert_voltage_to_temperature(pin_mv: u16) -> f32 {
    const V0C: f32 = 400.0; // Output voltage at 0°C in mV
    const TC: f32 = 19.5; // Temperature coefficient in mV/°C

    let voltage = pin_mv as f32; // Convert pin_mv to f32 for calculation
    let temperature_c = (voltage - V0C) / TC;

    temperature_c
}

#[main]
async fn main(spawner: Spawner) {
    println!("Starting program!...");

    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    let timg0 = TimerGroup::new_async(peripherals.TIMG0, &clocks);
    esp_hal_embassy::init(&clocks, timg0);

    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);

    let analog_pin = io.pins.gpio1;
    let sclk = io.pins.gpio6;
    let miso = io.pins.gpio8;
    let mosi = io.pins.gpio7;
    let cs = io.pins.gpio9;

    let mut adc1_config = AdcConfig::new();
    let adc1_pin =
        adc1_config.enable_pin_with_cal::<_, AdcCal>(analog_pin, Attenuation::Attenuation11dB);
    let adc1 = Adc::new(peripherals.ADC1, adc1_config);

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

    // Spawn the temperature task
    spawner.spawn(temperature_task(adc1, adc1_pin)).unwrap();
}
