#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod data;
mod hd108;
use data::VISUALIZATION_DATA;
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
use hd108::HD108;
use heapless::Vec;
use panic_halt as _;
use static_cell::StaticCell;

struct DriverInfo {
    number: u8,
    name: &'static str,
    team: &'static str,
    color: RGBColor,
}

struct RGBColor {
    r: u8,
    g: u8,
    b: u8,
}

/* Actual racing team colors
const driver_info: [DriverInfo; 20] = [
    DriverInfo {
        number: 1,
        name: "Max Verstappen",
        team: "Red Bull",
        color: RGBColor {
            r: 30,
            g: 65,
            b: 255,
        },
    },
    DriverInfo {
        number: 2,
        name: "Logan Sargeant",
        team: "Williams",
        color: RGBColor {
            r: 0,
            g: 82,
            b: 255,
        },
    },
    DriverInfo {
        number: 4,
        name: "Lando Norris",
        team: "McLaren",
        color: RGBColor {
            r: 255,
            g: 135,
            b: 0,
        },
    },
    DriverInfo {
        number: 10,
        name: "Pierre Gasly",
        team: "Alpine",
        color: RGBColor {
            r: 2,
            g: 144,
            b: 240,
        },
    },
    DriverInfo {
        number: 11,
        name: "Sergio Perez",
        team: "Red Bull",
        color: RGBColor {
            r: 30,
            g: 65,
            b: 255,
        },
    },
    DriverInfo {
        number: 14,
        name: "Fernando Alonso",
        team: "Aston Martin",
        color: RGBColor {
            r: 0,
            g: 110,
            b: 120,
        },
    },
    DriverInfo {
        number: 16,
        name: "Charles Leclerc",
        team: "Ferrari",
        color: RGBColor { r: 220, g: 0, b: 0 },
    },
    DriverInfo {
        number: 18,
        name: "Lance Stroll",
        team: "Aston Martin",
        color: RGBColor {
            r: 0,
            g: 110,
            b: 120,
        },
    },
    DriverInfo {
        number: 20,
        name: "Kevin Magnussen",
        team: "Haas",
        color: RGBColor {
            r: 160,
            g: 207,
            b: 205,
        },
    },
    DriverInfo {
        number: 22,
        name: "Yuki Tsunoda",
        team: "AlphaTauri",
        color: RGBColor {
            r: 60,
            g: 130,
            b: 200,
        },
    },
    DriverInfo {
        number: 23,
        name: "Alex Albon",
        team: "Williams",
        color: RGBColor {
            r: 0,
            g: 82,
            b: 255,
        },
    },
    DriverInfo {
        number: 24,
        name: "Zhou Guanyu",
        team: "Stake F1",
        color: RGBColor {
            r: 165,
            g: 160,
            b: 155,
        },
    },
    DriverInfo {
        number: 27,
        name: "Nico Hulkenberg",
        team: "Haas",
        color: RGBColor {
            r: 160,
            g: 207,
            b: 205,
        },
    },
    DriverInfo {
        number: 31,
        name: "Esteban Ocon",
        team: "Alpine",
        color: RGBColor {
            r: 2,
            g: 144,
            b: 240,
        },
    },
    DriverInfo {
        number: 40,
        name: "Liam Lawson",
        team: "AlphaTauri",
        color: RGBColor {
            r: 60,
            g: 130,
            b: 200,
        },
    },
    DriverInfo {
        number: 44,
        name: "Lewis Hamilton",
        team: "Mercedes",
        color: RGBColor {
            r: 0,
            g: 210,
            b: 190,
        },
    },
    DriverInfo {
        number: 55,
        name: "Carlos Sainz",
        team: "Ferrari",
        color: RGBColor { r: 220, g: 0, b: 0 },
    },
    DriverInfo {
        number: 63,
        name: "George Russell",
        team: "Mercedes",
        color: RGBColor {
            r: 0,
            g: 210,
            b: 190,
        },
    },
    DriverInfo {
        number: 77,
        name: "Valtteri Bottas",
        team: "Stake F1",
        color: RGBColor {
            r: 165,
            g: 160,
            b: 155,
        },
    },
    DriverInfo {
        number: 81,
        name: "Oscar Piastri",
        team: "McLaren",
        color: RGBColor {
            r: 255,
            g: 135,
            b: 0,
        },
    },
];

*/

// For testing purposes
const driver_info: [DriverInfo; 20] = [
    //Red
    DriverInfo {
        number: 1,
        name: "Max Verstappen",
        team: "Red Bull",
        color: RGBColor { r: 0, g: 0, b: 255 },
    },
    DriverInfo {
        number: 2,
        name: "Logan Sargeant",
        team: "Williams",
        color: RGBColor { r: 0, g: 0, b: 0 },
    },
    //Orange
    DriverInfo {
        number: 4,
        name: "Lando Norris",
        team: "McLaren",
        color: RGBColor {
            r: 242,
            g: 140,
            b: 40,
        },
    },
    DriverInfo {
        number: 10,
        name: "Pierre Gasly",
        team: "Alpine",
        color: RGBColor { r: 0, g: 0, b: 0 },
    },
    DriverInfo {
        number: 11,
        name: "Sergio Perez",
        team: "Red Bull",
        color: RGBColor {
            r: 210,
            g: 43,
            b: 43,
        },
    },
    //Orange
    DriverInfo {
        number: 14,
        name: "Fernando Alonso",
        team: "Aston Martin",
        color: RGBColor {
            r: 242,
            g: 140,
            b: 40,
        },
    },
    DriverInfo {
        number: 16,
        name: "Charles Leclerc",
        team: "Ferrari",
        color: RGBColor { r: 0, g: 0, b: 0 },
    },
    DriverInfo {
        number: 18,
        name: "Lance Stroll",
        team: "Aston Martin",
        color: RGBColor { r: 0, g: 0, b: 0 },
    },
    DriverInfo {
        number: 20,
        name: "Kevin Magnussen",
        team: "Haas",
        color: RGBColor { r: 0, g: 0, b: 0 },
    },
    DriverInfo {
        number: 22,
        name: "Yuki Tsunoda",
        team: "AlphaTauri",
        color: RGBColor { r: 0, g: 0, b: 0 },
    },
    DriverInfo {
        number: 23,
        name: "Alex Albon",
        team: "Williams",
        color: RGBColor { r: 0, g: 0, b: 0 },
    },
    DriverInfo {
        number: 24,
        name: "Zhou Guanyu",
        team: "Stake F1",
        color: RGBColor { r: 0, g: 0, b: 0 },
    },
    DriverInfo {
        number: 27,
        name: "Nico Hulkenberg",
        team: "Haas",
        color: RGBColor { r: 0, g: 0, b: 0 },
    },
    DriverInfo {
        number: 31,
        name: "Esteban Ocon",
        team: "Alpine",
        color: RGBColor { r: 0, g: 0, b: 0 },
    },
    DriverInfo {
        number: 40,
        name: "Liam Lawson",
        team: "AlphaTauri",
        color: RGBColor { r: 0, g: 0, b: 0 },
    },
    DriverInfo {
        number: 44,
        name: "Lewis Hamilton",
        team: "Mercedes",
        color: RGBColor { r: 0, g: 0, b: 0 },
    },
    DriverInfo {
        number: 55,
        name: "Carlos Sainz",
        team: "Ferrari",
        color: RGBColor { r: 0, g: 0, b: 0 },
    },
    //Green
    DriverInfo {
        number: 63,
        name: "George Russell",
        team: "Mercedes",
        color: RGBColor {
            r: 80,
            g: 200,
            b: 120,
        },
    },
    DriverInfo {
        number: 77,
        name: "Valtteri Bottas",
        team: "Stake F1",
        color: RGBColor { r: 0, g: 0, b: 0 },
    },
    DriverInfo {
        number: 81,
        name: "Oscar Piastri",
        team: "McLaren",
        color: RGBColor { r: 0, g: 0, b: 0 },
    },
];

enum Message {
    ButtonPressed,
}

static SIGNAL_CHANNEL: StaticCell<Channel<NoopRawMutex, Message, 1>> = StaticCell::new();

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

    // Spawn the button task with ownership of the button pin and the sender
    spawner
        .spawn(button_task(button_pin, signal_channel.sender()))
        .unwrap();

    // Spawn the run_race_task with the receiver
    spawner
        .spawn(run_race_task(hd108, signal_channel.receiver()))
        .unwrap();
}

/*
#[embassy_executor::task]
async fn led_task(
    mut hd108: HD108<impl SpiBus<u8> + 'static>,
    receiver: Receiver<'static, NoopRawMutex, Message, 1>,
) {
    loop {
        // Wait for the start message
        receiver.receive().await;
        for i in 0..=96 {
            let color = &DRIVER_COLORS[i % DRIVER_COLORS.len()]; // Get the corresponding color
            hd108.set_led(i, color.r, color.g, color.b).await.unwrap(); // Pass the RGB values directly

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
/*

#[embassy_executor::task]
async fn multi_led_task(
    mut hd108: HD108<impl SpiBus<u8> + 'static>,
    receiver: Receiver<'static, NoopRawMutex, Message, 1>,
    led_nums_and_colors: &'static [(usize, u8, u8, u8)],
) {
    loop {
        // Wait for the start message
        receiver.receive().await;

        // Set the specified LEDs to the given colors
        hd108.set_leds(led_nums_and_colors).await.unwrap();

        // Check for a stop message to turn off the LEDs
        if receiver.try_receive().is_ok() {
            hd108.set_off().await.unwrap();
            break;
        }

        Timer::after(Duration::from_millis(25)).await; // Debounce delay
    }
}
*/

#[embassy_executor::task]
async fn run_race_task(
    mut hd108: HD108<impl SpiBus<u8> + 'static>,
    receiver: Receiver<'static, NoopRawMutex, Message, 1>,
) {
    loop {
        // Wait for the start message
        receiver.receive().await;

        // Iterate through each frame in the visualization data
        for frame in &data::VISUALIZATION_DATA.frames {
            // Collect the LED updates for the current frame
            let mut led_updates: Vec<(usize, u8, u8, u8), 20> = Vec::new();

            for driver_data in frame.drivers.iter().flatten() {
                // Find the corresponding driver info
                if let Some(driver) = driver_info
                    .iter()
                    .find(|d| u32::from(d.number) == driver_data.driver_number)
                {
                    led_updates
                        .push((
                            driver_data.led_num.try_into().unwrap(),
                            driver.color.r,
                            driver.color.g,
                            driver.color.b,
                        ))
                        .unwrap();
                }
            }

            // Set the LEDs for the current frame
            hd108.set_leds(&led_updates).await.unwrap();

            // Wait for the update rate duration
            Timer::after(Duration::from_millis(
                data::VISUALIZATION_DATA.update_rate_ms as u64,
            ))
            .await;

            // Check for a stop message to turn off the LEDs
            if receiver.try_receive().is_ok() {
                hd108.set_off().await.unwrap();
                break;
            }
        }

        // Turn off LEDs after finishing the frames
        hd108.set_off().await.unwrap();
    }
}

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
