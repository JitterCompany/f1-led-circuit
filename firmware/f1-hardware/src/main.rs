#![no_std]
#![no_main]

mod hd108;

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
use panic_rtt_target as _;
use rtt_target::{rtt_init_print, rprintln};
use embassy_time::{Duration, Timer};
use embassy_executor::Spawner;
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_hal::spi::SpiDevice;
use embedded_hal::digital::{OutputPin, ErrorType};

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

    let mut spi = Spi::new(peripherals.SPI2, 100.kHz(), SpiMode::Mode0, &clocks)
        .with_pins(Some(sclk), None, Some(miso), None);

    // Assuming the async configuration method:
    dma_channel.configure(
        false,
        &mut descriptors,
        &mut rx_descriptors,
        DmaPriority::Priority0,
    );

    let spi_device = ExclusiveDevice::new_no_delay(spi, cs).unwrap();

    let mut hd108 = HD108::new(&mut spi_device);

    let send_buffer = [0, 1, 2, 3, 4, 5, 6, 7];
    loop {
        let mut buffer = [0; 8];
        rprintln!("Sending bytes");
        hd108.spi.transfer(&mut buffer, &send_buffer).unwrap();
        rprintln!("Bytes received: {:?}", buffer);
        Timer::after(Duration::from_millis(5_000)).await;
    }
}
