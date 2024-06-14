#![no_std]
#![no_main]

mod hd108;
mod spi_wrapper;


use esp_hal::{
    clock::ClockControl,
    gpio::Io,
    peripherals::Peripherals,
    prelude::*,
    spi::SpiMode,
    system::SystemControl,
};
use hd108::HD108;
use spi_wrapper::SpiWrapper;
use panic_rtt_target as _;
use rtt_target::{rtt_init_print, rprintln};
use riscv_rt::entry;
use embassy_executor::raw::Executor;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    // Get access to the device peripherals
    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    // Initialize the SPI interface
    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);
    
    let sclk = io.pins.gpio6;
    let miso = io.pins.gpio7;
    let mosi = io.pins.gpio13;
    let cs = io.pins.gpio10;

    let mut spi = esp_hal::spi::master::Spi::new(
        peripherals.SPI2,
        40.MHz(),
        SpiMode::Mode0,
        &mut clocks,
    )
    .with_pins(Some(sclk), Some(mosi), Some(miso), Some(cs));

    // Wrap the SPI interface
    let spi_wrapper = SpiWrapper::new(spi);

    // Create an instance of the HD108 driver
    let mut hd108 = HD108::new(spi_wrapper);


    let executor = Executor::new();

    executor.run(|spawner| {
        spawner.spawn(async {
            // Use the make_red function to set the first LED to red
            if let Err(e) = hd108.make_red().await {
                rprintln!("Error: {:?}", e);
            }
        }).unwrap();
    });

    loop {}
}
