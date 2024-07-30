#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

//mod data;
mod driver_info;
mod hd108;
use driver_info::{DriverInfo, DRIVERS};
//use data::VISUALIZATION_DATA;
use core::alloc::GlobalAlloc;
use core::alloc::Layout;
use core::fmt;
use core::mem::MaybeUninit;
use core::ptr::null_mut;
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
use grounded::uninit::GroundedCell;
use hd108::HD108;
use heapless08::Vec;
use panic_halt as _;
use postcard::from_bytes;
use serde::de::{self, Deserializer, SeqAccess, Visitor};
use serde::ser::{SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};
use static_cell::StaticCell;

// Define a simple global allocator using static mut
struct SimpleAllocator;

// Implement GlobalAlloc for our allocator
unsafe impl GlobalAlloc for SimpleAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        static mut ALLOCATOR: MaybeUninit<GroundedCell<[u8; 1024]>> = MaybeUninit::uninit();
        let allocator = ALLOCATOR.assume_init_mut();
        let ptr = allocator.get();
        if ptr.is_null() {
            null_mut()
        } else {
            ptr as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // No-op for this simple allocator
    }
}

#[global_allocator]
static GLOBAL: SimpleAllocator = SimpleAllocator;

impl Serialize for VisualizationData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.frames.len()))?;
        for frame in &self.frames {
            seq.serialize_element(frame)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for VisualizationData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct FrameVisitor;

        impl<'de> Visitor<'de> for FrameVisitor {
            type Value = [UpdateFrame; 8879];

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an array of 8879 UpdateFrame")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                // Create an array of MaybeUninit for uninitialized memory
                let mut frames: [MaybeUninit<UpdateFrame>; 8879] = unsafe {
                    // SAFETY: An uninitialized `[MaybeUninit<UpdateFrame>; 8879]` is valid.
                    MaybeUninit::uninit().assume_init()
                };

                for i in 0..8879 {
                    frames[i] = MaybeUninit::new(
                        seq.next_element()?
                            .ok_or_else(|| de::Error::invalid_length(i, &self))?,
                    );
                }

                // SAFETY: All elements are initialized at this point
                let frames = unsafe { core::mem::transmute::<_, [UpdateFrame; 8879]>(frames) };
                Ok(frames)
            }
        }

        let frames = deserializer.deserialize_seq(FrameVisitor)?;
        Ok(VisualizationData {
            update_rate_ms: 250, // Set this according to your data
            frames,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DriverData {
    pub driver_number: u8,
    pub led_num: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateFrame {
    pub frame: [Option<DriverData>; 20],
}

#[derive(Debug)]
pub struct VisualizationData {
    pub update_rate_ms: u32,
    pub frames: [UpdateFrame; 8879],
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

/*  OLD
#[embassy_executor::task]
async fn run_race_task(
    mut hd108: HD108<impl SpiBus<u8> + 'static>,
    receiver: Receiver<'static, NoopRawMutex, ButtonMessage, 1>,
) {

    loop {
        match receiver.receive().await {
            ButtonMessage::ButtonPressed => {
                println!("Button pressed, starting race...");

                // Load and deserialize the binary data
                let data_bin = include_bytes!("data.bin");
                let visualization_data: VisualizationData = from_bytes(data_bin).unwrap();

                for frame in &visualization_data.frames {
                    let mut led_updates: heapless08::Vec<(usize, u8, u8, u8), 20> = heapless08::Vec::new();

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

                    hd108.set_leds(&led_updates).await.unwrap();

                    Timer::after(Duration::from_millis(
                        visualization_data.update_rate_ms as u64,
                    ))
                    .await;

                    if receiver.try_receive().is_ok() {
                        hd108.set_off().await.unwrap();
                        break;
                    }
                }

                hd108.set_off().await.unwrap();
            }
        }
    }
}

*/

/*
#[embassy_executor::task]
async fn led_task(
    mut hd108: HD108<impl SpiBus<u8> + 'static>,
    receiver: Receiver<'static, NoopRawMutex, ButtonMessage, 1>,
) {
    loop {
        // Wait for the start message
        receiver.receive().await;
        for i in 0..=96 {
            let color = DRIVER_COLORS[i % DRIVER_COLORS.len()];
            hd108.set_led(i, color.0, color.1, color.2).await.unwrap();

            // Check for a stop message
            if receiver.try_receive().is_ok() {
                hd108.set_off().await.unwrap();
                break;
            }
            Timer::after(Duration::from_millis(25)).await; // Debounce delay
        }
    }
}
*/

#[embassy_executor::task]
async fn led_task(
    mut hd108: HD108<impl SpiBus<u8> + 'static>,
    receiver: Receiver<'static, NoopRawMutex, ButtonMessage, 1>,
) {


    loop {
        // Wait for the start message
        receiver.receive().await;

        println!("Button pressed, starting race...");
        
        // Load and deserialize the binary data
        let data_bin = include_bytes!("data.bin");
        let visualization_data: VisualizationData = match from_bytes(data_bin) {

            Ok(data) => data,
            Err(err) => {
                println!("Failed to deserialize data: {:?}", err);
                continue; // Skip this iteration if deserialization fails
            }
        };
        

        for frame in &visualization_data.frames {
            let mut led_updates: heapless08::Vec<(usize, u8, u8, u8), 20> = heapless08::Vec::new();

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

            if let Err(err) = hd108.set_leds(&led_updates).await {
                println!("Failed to set LEDs: {:?}", err);
            }

            Timer::after(Duration::from_millis(
                visualization_data.update_rate_ms as u64,
            ))
            .await;

            if receiver.try_receive().is_ok() {
                hd108.set_off().await.unwrap();
                break;
            }
        }

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

const DRIVER_COLORS: [(u8, u8, u8); 20] = {
    let mut colors = [(0, 0, 0); 20];
    let mut i = 0;
    while i < DRIVERS.len() {
        colors[i] = DRIVERS[i].color;
        i += 1;
    }
    colors
};
