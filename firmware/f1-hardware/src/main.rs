#![no_std]
#![no_main]

mod hd108;

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::{
    clock::ClockControl,
    dma::{Dma, DmaPriority},
    dma_descriptors,
    gpio::Io,
    peripherals::Peripherals,
    prelude::*,
    spi::{master::Spi, SpiMode},
    system::SystemControl,
    timer::timg::TimerGroup,
};
use hd108::HD108;
use heapless::Vec;
use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};
//use embedded_hal_async::spi::SpiBus;
//use embedded_hal::digital::{OutputPin, ErrorType};
use esp_hal::spi::master::prelude::_esp_hal_spi_master_dma_WithDmaSpi2;

struct RGBColor {
    r: u8,
    g: u8,
    b: u8,
}

const DRIVER_COLORS: [RGBColor; 20] = [
    RGBColor {
        r: 30,
        g: 65,
        b: 255,
    }, // Max Verstappen
    RGBColor {
        r: 0,
        g: 82,
        b: 255,
    }, // Logan Sargeant
    RGBColor {
        r: 255,
        g: 135,
        b: 0,
    }, // Lando Norris
    RGBColor {
        r: 2,
        g: 144,
        b: 240,
    }, // Pierre Gasly
    RGBColor {
        r: 30,
        g: 65,
        b: 255,
    }, // Sergio Perez
    RGBColor {
        r: 0,
        g: 110,
        b: 120,
    }, // Fernando Alonso
    RGBColor { r: 220, g: 0, b: 0 }, // Charles Leclerc
    RGBColor {
        r: 0,
        g: 110,
        b: 120,
    }, // Lance Stroll
    RGBColor {
        r: 160,
        g: 207,
        b: 205,
    }, // Kevin Magnussen
    RGBColor {
        r: 60,
        g: 130,
        b: 200,
    }, // Yuki Tsunoda
    RGBColor {
        r: 0,
        g: 82,
        b: 255,
    }, // Alex Albon
    RGBColor {
        r: 165,
        g: 160,
        b: 155,
    }, // Zhou Guanyu
    RGBColor {
        r: 160,
        g: 207,
        b: 205,
    }, // Nico Hulkenberg
    RGBColor {
        r: 2,
        g: 144,
        b: 240,
    }, // Esteban Ocon
    RGBColor {
        r: 60,
        g: 130,
        b: 200,
    }, // Liam Lawson
    RGBColor {
        r: 0,
        g: 210,
        b: 190,
    }, // Lewis Hamilton
    RGBColor { r: 220, g: 0, b: 0 }, // Carlos Sainz
    RGBColor {
        r: 0,
        g: 210,
        b: 190,
    }, // George Russell
    RGBColor {
        r: 165,
        g: 160,
        b: 155,
    }, // Valtteri Bottas
    RGBColor {
        r: 255,
        g: 135,
        b: 0,
    }, // Oscar Piastri
];

/* *
struct _DummyPin;

impl ErrorType for DummyPin {
    type Error = core::convert::Infallible;
}

impl OutputPin for DummyPin {
    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
*/

#[main]
async fn main(_spawner: Spawner) {
    rtt_init_print!();
    rprintln!("Starting program!...");

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

    let (mut descriptors, mut rx_descriptors) = dma_descriptors!(32000);

    let mut spi = Spi::new(peripherals.SPI2, 20.MHz(), SpiMode::Mode0, &clocks)
        .with_pins(Some(sclk), Some(mosi), Some(miso), Some(cs))
        .with_dma(dma_channel.configure_for_async(
            false,
            &mut descriptors,
            &mut rx_descriptors,
            DmaPriority::Priority0,
        ));

    let mut hd108 = HD108::new(&mut spi);

    loop {
        for i in 0..96 {
            let color = &DRIVER_COLORS[i % DRIVER_COLORS.len()]; // Get the corresponding color
            let mut rgb_vec: Vec<u8, 3> = Vec::new();
            rgb_vec.push(color.r).unwrap();
            rgb_vec.push(color.g).unwrap();
            rgb_vec.push(color.b).unwrap();
            hd108.set_led(i, rgb_vec).await.unwrap();
            Timer::after(Duration::from_millis(100)).await;
        }
    }
}
