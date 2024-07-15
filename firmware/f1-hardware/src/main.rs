#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod data;
mod hd108;
mod driver_info;
use driver_info::DRIVERS;
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
    rng::Rng,
};
use esp_println::println;
use hd108::HD108;
use heapless::Vec;
use panic_halt as _;
use static_cell::StaticCell;

// Wifi
use embassy_net::{tcp::TcpSocket, Config, Ipv4Address, Stack, StackResources};
use esp_wifi::{
    initialize,
    wifi::{
        ClientConfiguration, Configuration, WifiController, WifiDevice, WifiEvent, WifiStaDevice,
        WifiState,
    },
    EspWifiInitFor,
};

macro_rules! mk_static {
    ($t:path,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

const SSID: &str = "NOS-C8C0 Plumes";
const PASSWORD: &str = "AGYQ24VR";

enum Message {
    ButtonPressed,
    WifiConnected,
}

static SIGNAL_CHANNEL: StaticCell<Channel<NoopRawMutex, Message, 1>> = StaticCell::new(); // Updated capacity to 1

#[main]
async fn main(spawner: Spawner) {
    println!("Starting program!...");

    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);
    let clocks = ClockControl::max(system.clock_control).freeze();

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
    spawner.spawn(button_task(button_pin, signal_channel.sender())).unwrap();

    // Spawn the run_race_task with the receiver
    spawner.spawn(run_race_task(hd108, signal_channel.receiver())).unwrap();

    // Wifi
    let timer = esp_hal::timer::systimer::SystemTimer::new(peripherals.SYSTIMER).alarm0;

    println!("Initializing WiFi...");

    match initialize(
        EspWifiInitFor::Wifi,
        timer,
        Rng::new(peripherals.RNG),
        peripherals.RADIO_CLK,
        &clocks,
    ) {
        Ok(init_wifi) => {
            println!("WiFi initialized...");
            let wifi = peripherals.WIFI;
            match esp_wifi::wifi::new_with_mode(&init_wifi, wifi, WifiStaDevice) {
                Ok((wifi_interface, controller)) => {
                    println!("WiFi controller and interface created...");
                    // Continue with the rest of the setup

                    let config = Config::dhcpv4(Default::default());
                    let seed = 1234; // very random, very secure seed

                    // Init network stack
                    let stack = &*mk_static!(
                        Stack<WifiDevice<'_, WifiStaDevice>>,
                        Stack::new(
                            wifi_interface,
                            config,
                            mk_static!(StackResources<3>, StackResources::<3>::new()),
                            seed
                        )
                    );

                    println!("Spawning connection...");

                    spawner.spawn(connection(controller, stack, signal_channel.sender())).ok();
                    spawner.spawn(net_task(stack)).ok();
                    spawner.spawn(fetch_update_frames(signal_channel.receiver(), stack)).ok(); // Corrected line
                }
                Err(e) => {
                    println!("Failed to create WiFi controller and interface: {:?}", e);
                }
            }
        }
        Err(e) => {
            println!("Failed to initialize WiFi: {:?}", e);
        }
    }
}

#[embassy_executor::task]
async fn run_race_task(
    mut hd108: HD108<impl SpiBus<u8> + 'static>,
    receiver: Receiver<'static, NoopRawMutex, Message, 1>,
) {
    loop {
        // Wait for the ButtonPressed message
        if let Message::ButtonPressed = receiver.receive().await {
            // Iterate through each frame in the visualization data
            for frame in &data::VISUALIZATION_DATA.frames {
                // Collect the LED updates for the current frame
                let mut led_updates: Vec<(usize, u8, u8, u8), 20> = Vec::new();

                for driver_data in frame.drivers.iter().flatten() {
                    // Find the corresponding driver info
                    if let Some(driver) = DRIVERS
                        .iter()
                        .find(|d| d.number == driver_data.driver_number)
                    {
                        led_updates
                            .push((
                                driver_data.led_num.try_into().unwrap(),
                                driver.color.0,
                                driver.color.1,
                                driver.color.2,
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

#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>, stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>, sender: Sender<'static, NoopRawMutex, Message, 1>) {
    println!("start connection task");
    println!("Device capabilities: {:?}", controller.get_capabilities());
    loop {
        match esp_wifi::wifi::get_wifi_state() {
            WifiState::StaConnected => {
                // wait until we're no longer connected
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after(Duration::from_millis(5000)).await
            }
            _ => {}
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.try_into().unwrap(),
                password: PASSWORD.try_into().unwrap(),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            println!("Starting wifi");
            controller.start().await.unwrap();
            println!("Wifi started!");
        }
        println!("About to connect...");

        match controller.connect().await {
            Ok(_) => {
                println!("Wifi connected!");
                // Wait for an IP address
                loop {
                    if let Some(config) = stack.config_v4() {
                        println!("Got IP: {}", config.address);
                        sender.send(Message::WifiConnected).await;
                        break;
                    }
                    Timer::after(Duration::from_millis(500)).await;
                }
            },
            Err(e) => {
                println!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>) {
    stack.run().await
}

#[embassy_executor::task]
async fn fetch_update_frames(receiver: Receiver<'static, NoopRawMutex, Message, 1>, stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>) {
    loop {
        // Wait for the WifiConnected message
        if let Message::WifiConnected = receiver.receive().await {
            println!("Fetching update frames started...");

            // Connect to 8.8.8.8
            let mut rx_buffer = [0; 4096];
            let mut tx_buffer = [0; 4096];

            let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
            socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

            let remote_endpoint = (Ipv4Address::new(8, 8, 8, 8), 80);
            println!("connecting to Google...");
            let r = socket.connect(remote_endpoint).await;
            if let Err(e) = r {
                println!("connect error: {:?}", e);
                continue;
            }
            println!("Connected to Google..");
        }
    }
}