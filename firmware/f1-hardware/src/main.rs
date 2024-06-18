#![no_std]
#![no_main]

mod hd108;

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
use panic_rtt_target as _;
use rtt_target::{rtt_init_print, rprintln};
use embassy_time::{Duration, Timer};
use embassy_executor::Spawner;
//use embedded_hal_async::spi::SpiBus;
//use embedded_hal::digital::{OutputPin, ErrorType};
use esp_hal::spi::master::prelude::_esp_hal_spi_master_dma_WithDmaSpi2;

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

    let mut spi = Spi::new(peripherals.SPI2, 100.kHz(), SpiMode::Mode0, &clocks)
        .with_pins(Some(sclk), Some(mosi), Some(miso), Some(cs))
        .with_dma(dma_channel.configure_for_async(
            false,
            &mut descriptors,
            &mut rx_descriptors,
            DmaPriority::Priority0,
        ));

        let mut hd108 = HD108::new(& mut spi);

        loop {
            rprintln!("Making LED red...");
            HD108::make_red(& mut hd108)
            .await
            .unwrap();
            Timer::after(Duration::from_millis(5_000)).await;
        }
    }
