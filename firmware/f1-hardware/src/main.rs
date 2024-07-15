#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod data;
mod hd108;
mod driver_info;
use driver_info::DRIVERS;
use data::VISUALIZATION_DATA;

use embassy_net::dns::{DnsSocket, DnsQueryType};
use embassy_net::Stack;
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
use panic_halt as _;
use static_cell::StaticCell;
use embedded_io_async::Write;
use heapless::{String, Vec};
use serde::{Deserialize, Serialize};
use postcard::{to_vec, from_bytes};

// Wifi
use embassy_net::{tcp::TcpSocket, Config, StackResources};
use esp_wifi::{
    initialize,
    wifi::{
        ClientConfiguration, Configuration, WifiController, WifiDevice, WifiEvent, WifiStaDevice,
        WifiState,
    },
    EspWifiInitFor,
    wifi::get_sta_state,
};

macro_rules! mk_static {
    ($t:path,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");


#[derive(Debug, Clone, Serialize, Deserialize)]
struct FetchedData {
    date: String<32>,
    driver_number: u32,
    meeting_key: u32,
    session_key: u32,
    x: i32,
    y: i32,
    z: i32,
}

impl FetchedData {
    fn from_postcard(bytes: &[u8]) -> Result<Self, &'static str> {
        from_bytes(bytes).map_err(|_| "Failed to deserialize")
    }

    fn to_postcard(&self) -> Result<Vec<u8, 128>, &'static str> {
        to_vec(self).map_err(|_| "Failed to serialize")
    }
}


enum ButtonMessage {
    ButtonPressed,
}

enum WifiMessage {
    WifiConnected,
}


enum FetchMessage {
    FetchedData([FetchedData; 2]), // Fixed-size array for the fetched data
}

static BUTTON_CHANNEL: StaticCell<Channel<NoopRawMutex, ButtonMessage, 1>> = StaticCell::new();
static WIFI_CHANNEL: StaticCell<Channel<NoopRawMutex, WifiMessage, 1>> = StaticCell::new();
static FETCH_CHANNEL: StaticCell<Channel<NoopRawMutex, FetchMessage, 1>> = StaticCell::new();

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

    let button_channel = BUTTON_CHANNEL.init(Channel::new());
    let wifi_channel = WIFI_CHANNEL.init(Channel::new());
    let fetch_channel = FETCH_CHANNEL.init(Channel::new());

    // Spawn the button task with ownership of the button pin and the sender
    spawner.spawn(button_task(button_pin, button_channel.sender())).unwrap();

    // Spawn the run_race_task with the receiver
    spawner.spawn(run_race_task(hd108, button_channel.receiver())).unwrap();

    spawner.spawn(store_data(fetch_channel.receiver())).ok();


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

                    println!("Spawning wifi connection...");

                    spawner.spawn(wifi_connection(controller, stack, wifi_channel.sender())).ok();
                    spawner.spawn(net_task(stack)).ok();
                    spawner.spawn(fetch_update_frames(wifi_channel.receiver(), stack)).ok();
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
    receiver: Receiver<'static, NoopRawMutex, ButtonMessage, 1>,
) {
    loop {
        match receiver.receive().await {
            ButtonMessage::ButtonPressed => {
                println!("Button pressed, starting race...");
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
        println!("Button pressed, message sent.");
        Timer::after(Duration::from_millis(400)).await; // Debounce delay
    }
}

#[embassy_executor::task]

async fn wifi_connection(
    mut controller: WifiController<'static>,
    stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>,
    sender: Sender<'static, NoopRawMutex, WifiMessage, 1>,
) {
    println!("start wifi connection task");
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
                        sender.send(WifiMessage::WifiConnected).await;
                        break;
                    }
                    Timer::after(Duration::from_millis(500)).await;
                }
            }
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
async fn fetch_update_frames(
    receiver: Receiver<'static, NoopRawMutex, WifiMessage, 1>,
    stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>,
    sender: Sender<'static, NoopRawMutex, FetchMessage, 1>,
) {
    let dns_socket = DnsSocket::new(stack);
    let hostname = "api.openf1.org"; // Replace with your hostname
) {
    loop {
        // Wait for the WifiConnected message
        match receiver.receive().await {
            WifiMessage::WifiConnected => {
                println!("Fetching update frames started...");

                // Resolve the hostname to an IP address
                match dns_socket.query(hostname, DnsQueryType::A).await {
                    Ok(ip_addresses) => {
                        if let Some(ip_address) = ip_addresses.get(0) {
                            // Use the first IP address returned
                            let remote_endpoint = (*ip_address, 80);

                            // Connect to the resolved IP address
                            let mut rx_buffer = [0; 4096];
                            let mut tx_buffer = [0; 4096];
                            let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
                            socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

                            println!("Connecting to {}...", hostname);
                            match socket.connect(remote_endpoint).await {
                                Ok(_) => {
                                    println!("Connected to {}..", hostname);
                                    // Fetch data from the URL
                                    let url = "GET /v1/location?session_key=9161&driver_number=81&date>2023-09-16T13:03:35.200&date<2023-09-16T13:03:35.800 HTTP/1.1\r\nHost: api.openf1.org\r\n\r\n";
                                    socket.write_all(url.as_bytes()).await.unwrap();

                                    let mut response = [0u8; 2048];
                                    let n = socket.read(&mut response).await.unwrap();

                                    let fetched_data = FetchedData::from_postcard(&response[..n]);
                                    match fetched_data {
                                        Ok(data) => {
                                            sender.send(FetchMessage::FetchedData([data.clone(), data])).await;
                                        }
                                        Err(e) => {
                                            println!("Failed to parse response: {:?}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("Connect error: {:?}", e);
                                }
                            }
                        } else {
                            println!("No IP addresses found for {}", hostname);
                        }
                    }
                    Err(e) => {
                        println!("DNS resolve error: {:?}", e);
                    }
                }
            }
        }
    }
}

#[embassy_executor::task]
async fn store_data(receiver: Receiver<'static, NoopRawMutex, FetchMessage, 1>) {
    let mut data_to_be_visualized: Option<[FetchedData; 2]> = None;

    loop {
        match receiver.receive().await {
            FetchMessage::FetchedData(data) => {
                println!("Received data: {:?}", data);
                data_to_be_visualized = Some(data);
                // Perform any additional processing if necessary
            }
        }
    }
}
