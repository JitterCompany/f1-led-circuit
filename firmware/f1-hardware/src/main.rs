#![no_std]
#![no_main]

mod hd108;
mod spiwrapper;

use esp_hal::{
    clock::ClockControl,
    gpio::Io,
    peripherals::Peripherals,
    prelude::*,
    spi::{master::Spi, SpiMode},
    system::SystemControl,
};
use hd108::HD108;
use panic_rtt_target as _;
use rtt_target::{rtt_init_print, rprintln};
use embassy_executor::raw::Executor;
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_hal::digital::{OutputPin,ErrorType};
use spiwrapper::AsyncSpiBusWrapper;

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

#[riscv_rt::entry]
fn main() -> ! {
    rtt_init_print!();
    rprintln!("Starting program!...");

    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();
    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);

    let sclk = io.pins.gpio6;
    let miso = io.pins.gpio7;
    let mosi = DummyPin;
    let cs = DummyPin.set_high();
    let spi = Spi::new(
        peripherals.SPI2,
        100.kHz(),
        SpiMode::Mode0,
        &clocks,
    )
    .with_pins(Some(sclk), None, Some(miso), None);
    let cs_pin = cs;

    let spi_device = ExclusiveDevice::new_no_delay(spi, DummyPin);
    let async_spi_device = AsyncSpiBusWrapper::new(spi_device);
    let mut hd108 = HD108::new(&mut async_spi_device);

    let executor = Executor::new(core::ptr::null_mut());

    executor.run(async {
        hd108.make_red().await.unwrap();
    });

    loop {
        riscv::asm::wfi();
    }
}
