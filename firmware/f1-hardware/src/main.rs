#![no_std]
#![no_main]

mod hd108;

use esp_hal::{
    clock::ClockControl,
    gpio::Io,
    peripherals::Peripherals,
    prelude::*,
    spi::{master::Spi, SpiMode, Error},
    system::SystemControl,
};
use hd108::HD108;
use panic_rtt_target as _;
use rtt_target::{rtt_init_print, rprintln};
use riscv_rt::entry;
use embedded_hal_async::spi::SpiBus;

struct SpiWrapper<'a, T> {
    spi: &'a mut Spi<'a, T, esp_hal::spi::FullDuplexMode>,
}

impl<'a, T> SpiWrapper<'a, T> {
    fn new(spi: &'a mut Spi<'a, T, esp_hal::spi::FullDuplexMode>) -> Self {
        Self { spi }
    }
}

impl<'a, T> SpiBus<u8> for SpiWrapper<'a, T>
where
    T: esp_hal::spi::Instance,
{
    //type Error = Error;

    async fn read(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        for word in words.iter_mut() {
            *word = self.spi.read_byte()?;
        }
        Ok(())
    }

    async fn write(&mut self, words: &[u8]) -> Result<(), Self::Error> {
        self.spi.write_bytes(words)
    }

    async fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Self::Error> {
        for (read_byte, write_byte) in read.iter_mut().zip(write.iter()) {
            self.spi.write_byte(*write_byte)?;
            *read_byte = self.spi.read_byte()?;
        }
        Ok(())
    }

    async fn transfer_in_place(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        for word in words.iter_mut() {
            let write_byte = *word;
            self.spi.write_byte(write_byte)?;
            *word = self.spi.read_byte()?;
        }
        Ok(())
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        // Assuming flush means to wait until the SPI bus is idle
        // This can be a no-op if the HAL does not provide such functionality
        Ok(())
    }
}

#[entry]
fn main() -> ! {
    rtt_init_print!();

    rprintln!("Starting program!...")

    let peripherals = Peripherals::take().unwrap();
    let system = SystemControl::new(peripherals.SYSTEM);
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);
    
    let sclk = io.pins.gpio6;
    let miso = io.pins.gpio7;
    let mosi = io.pins.gpio13;
    let cs = io.pins.gpio10;

    let mut spi = Spi::new(
        peripherals.SPI2,
        100.kHz(),
        SpiMode::Mode0,
        &mut clocks,
    )
    .with_pins(Some(sclk), Some(mosi), Some(miso), Some(cs));

    let mut spi_wrapper = SpiWrapper::new(&mut spi);

    let mut hd108 = HD108::new(&mut spi_wrapper);

    // Placeholder for running async function
    // Here you would run your executor to poll the async functions
    // For example: executor.run(async { hd108.make_red().await.unwrap(); });

    loop {}
}
