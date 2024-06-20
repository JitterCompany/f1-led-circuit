#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod hd108;
use hd108::HD108;

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::channel::Receiver;
use embassy_sync::channel::Sender;
use embassy_sync::signal::Signal;
use core::sync::atomic::{AtomicBool, Ordering};
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
use panic_halt as _;
use static_cell::StaticCell;


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

static SIGNAL_CHANNEL: StaticCell<Channel<NoopRawMutex, (), 1>> = StaticCell::new();
static RUNNING_STATE: StaticCell<AtomicBool> = StaticCell::new();
static SIGNAL: StaticCell<Signal<NoopRawMutex, ()>> = StaticCell::new();

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
    let running_state = RUNNING_STATE.init(AtomicBool::new(false));
    let signal = SIGNAL.init(Signal::new());

    // Spawn the button task with ownership of the button pin and the sender
    spawner.spawn(button_task(button_pin, signal_channel.sender(), running_state, signal)).unwrap();

    // Spawn the led task with the receiver
    spawner.spawn(led_task(hd108, signal_channel.receiver(), running_state, signal)).unwrap();
}

#[embassy_executor::task]
async fn led_task(
    mut hd108: HD108<impl SpiBus<u8> + 'static>,
    receiver: Receiver<'static, NoopRawMutex, (), 1>,
    running_state: &'static AtomicBool,
    signal: &'static Signal<NoopRawMutex, ()>,
) {
    receiver.receive().await;

    let is_running = running_state.load(Ordering::SeqCst);

    if is_running {
        println!("Starting LED task...");
        for i in 0..96 {
            let color = &DRIVER_COLORS[i % DRIVER_COLORS.len()]; // Get the corresponding color
            hd108.set_led(i, color.r, color.g, color.b).await.unwrap(); // Pass the RGB values directly
            Timer::after(Duration::from_millis(100)).await; // Wait for 100 milliseconds
        }
    } else {
        println!("LED task is not running...");
    }

    signal.signal(());
}

#[embassy_executor::task]
async fn button_task(
    mut button_pin: Input<'static, GpioPin<10>>,
    sender: Sender<'static, NoopRawMutex, (), 1>,
    running_state: &'static AtomicBool,
    signal: &'static Signal<NoopRawMutex, ()>,
) {
    loop {
        button_pin.wait_for_falling_edge().await;
        println!("Button pressed...");
        Timer::after(Duration::from_millis(100)).await; // Debounce delay

        // Toggle the running state
        let new_state = !running_state.load(Ordering::SeqCst);
        running_state.store(new_state, Ordering::SeqCst);
        println!("Running state: {}", new_state);

        // Send signal to start/stop LED task
        sender.send(()).await;

        // Wait for LED task to process the state change
        signal.wait().await;
    }
}