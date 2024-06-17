#![no_std]
#![no_main]

mod hd108;
mod spi_wrapper;

use esp_hal::{
    clock::ClockControl,
    dma::*,
    dma_descriptors,
    gpio::Io,
    peripherals::Peripherals,
    prelude::*,
    spi::{master::Spi, SpiMode},
    system::SystemControl,
    timer::timg::TimerGroup,
};
use hd108::HD108;
use spi_wrapper::AsyncSpiDma;
use core::marker::Sized;
use panic_rtt_target as _;
use rtt_target::{rtt_init_print, rprintln};
use embassy_time::{Duration, Timer};
use embassy_executor::Spawner;
use embedded_hal::spi::SpiBus;
//use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_hal::digital::{OutputPin, ErrorType};
use esp_hal::spi::master::prelude::_esp_hal_spi_master_dma_WithDmaSpi2;

struct DummyPin;

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



#[main]
async fn main(spawner: Spawner) {
    rtt_init_print!();
    rprintln!("Starting program!...");

    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();
    
    let timg0 = TimerGroup::new_async(peripherals.TIMG0, &clocks);
    esp_hal_embassy::init(&clocks, timg0);

    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);

    let sclk = io.pins.gpio6;
    let miso = io.pins.gpio7;
    let mosi = DummyPin;
    let cs = DummyPin;

    let dma = Dma::new(peripherals.DMA);

    let dma_channel = dma.channel0;

    let (mut descriptors, mut rx_descriptors) = dma_descriptors!(32000);
    
    let tx_channel = dma.channel0.configure(false, &mut descriptors, &mut descriptors, DmaPriority::Priority0);
    let rx_channel = dma.channel1.configure(true, &mut rx_descriptors, &mut rx_descriptors, DmaPriority::Priority0);
    

    let spi = Spi::new(peripherals.SPI2, 100.kHz(), SpiMode::Mode0, &clocks)
        .with_pins(Some(sclk), None, Some(miso), None);

    let mut async_spi = AsyncSpiDma::new(spi, tx_channel, rx_channel);

    let mut hd108 = HD108::new(&mut async_spi);

    let send_buffer = [0, 1, 2, 3, 4, 5, 6, 7];
    loop {
        let mut buffer = [0; 8];
        rprintln!("Sending bytes");
        hd108.spi.transfer(&mut buffer, &send_buffer).unwrap();
        rprintln!("Bytes received: {:?}", buffer);
        Timer::after(Duration::from_millis(5_000)).await;
    }
}
