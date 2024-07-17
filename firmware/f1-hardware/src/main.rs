#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod data;
mod driver_info;
mod hd108;
use data::VISUALIZATION_DATA;
use driver_info::DRIVERS;

use core::fmt::Write as FmtWrite;
use embassy_executor::Spawner;
use embassy_net::dns::{DnsQueryType, DnsSocket};
use embassy_net::{Stack, Ipv4Address, Ipv4Cidr, IpAddress};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::channel::Receiver;
use embassy_sync::channel::Sender;
use embassy_time::{Duration, Timer};
use embedded_hal_async::spi::SpiBus;
use embedded_io_async::Write;
use esp_backtrace as _;
use esp_hal::dma::DmaDescriptor;
use esp_hal::spi::master::prelude::_esp_hal_spi_master_dma_WithDmaSpi2;
use esp_hal::{
    clock::ClockControl,
    dma::{Dma, DmaPriority},
    gpio::{Event, GpioPin, Input, Io, Pull},
    peripherals::Peripherals,
    prelude::*,
    rng::Rng,
    spi::{master::Spi, SpiMode},
    system::SystemControl,
    timer::timg::TimerGroup,
};
use esp_println::println;
use hd108::HD108;
use heapless::{String, Vec};
use panic_halt as _;
use postcard::from_bytes;
use postcard::to_vec;
use serde::{Deserialize, Serialize};
use serde_json_core::from_slice;
use static_cell::StaticCell;

// WiFi
use embassy_net::{tcp::TcpSocket, Config, StackResources};
use esp_wifi::{
    initialize,
    wifi::{
        ClientConfiguration, Configuration, WifiController, WifiDevice, WifiEvent, WifiStaDevice,
        WifiState,
    },
    EspWifiInitFor,
};

// Macro to create a static cell for the stack
macro_rules! mk_static {
    ($t:path,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

// WiFi credentials
const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");

// Struct to hold fetched data
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
    // Deserialize data from bytes
    fn from_postcard(bytes: &[u8]) -> Result<Self, &'static str> {
        from_bytes(bytes).map_err(|_| "Failed to deserialize")
    }

    // Serialize data to bytes
    fn to_postcard(&self) -> Result<Vec<u8, 128>, &'static str> {
        to_vec(self).map_err(|_| "Failed to serialize")
    }
}

// Enum to handle button messages
enum ButtonMessage {
    ButtonPressed,
}

// Enum to handle WiFi messages
enum WifiMessage {
    WifiConnected,
}

// Enum to handle fetch messages
enum FetchMessage {
    FetchedData(Vec<FetchedData, 64>),
}

// Static channels for button, WiFi, and fetch messages
static BUTTON_CHANNEL: StaticCell<Channel<NoopRawMutex, ButtonMessage, 1>> = StaticCell::new();
static WIFI_CHANNEL: StaticCell<Channel<NoopRawMutex, WifiMessage, 1>> = StaticCell::new();
static FETCH_CHANNEL: StaticCell<Channel<NoopRawMutex, FetchMessage, 1>> = StaticCell::new();

// Main function to initialize and spawn tasks
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
    button_pin.listen(Event::FallingEdge);

    let button_channel = BUTTON_CHANNEL.init(Channel::new());
    let wifi_channel = WIFI_CHANNEL.init(Channel::new());
    let fetch_channel = FETCH_CHANNEL.init(Channel::new());

    // Spawn tasks for button, race, and data storage
    spawner.spawn(button_task(button_pin, button_channel.sender())).unwrap();
    spawner.spawn(run_race_task(hd108, button_channel.receiver())).unwrap();
    spawner.spawn(store_data(fetch_channel.receiver())).ok();

    // WiFi initialization and connection
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

                    let config = Config::dhcpv4(Default::default());
                    let seed = 1234;

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
                    spawner.spawn(fetch_update_frames(
                        spawner,
                        wifi_channel.receiver(),
                        stack,
                        fetch_channel.sender(),
                    )).ok();
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

// Task to handle the race simulation
#[embassy_executor::task]
async fn run_race_task(
    mut hd108: HD108<impl SpiBus<u8> + 'static>,
    receiver: Receiver<'static, NoopRawMutex, ButtonMessage, 1>,
) {
    loop {
        match receiver.receive().await {
            ButtonMessage::ButtonPressed => {
                println!("Button pressed, starting race...");
                for frame in &data::VISUALIZATION_DATA.frames {
                    let mut led_updates: Vec<(usize, u8, u8, u8), 20> = Vec::new();
                    for driver_data in frame.drivers.iter().flatten() {
                        if let Some(driver) = DRIVERS.iter().find(|d| d.number == driver_data.driver_number) {
                            led_updates.push((
                                driver_data.led_num.try_into().unwrap(),
                                driver.color.0,
                                driver.color.1,
                                driver.color.2,
                            )).unwrap();
                        }
                    }
                    hd108.set_leds(&led_updates).await.unwrap();
                    Timer::after(Duration::from_millis(data::VISUALIZATION_DATA.update_rate_ms as u64)).await;
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

// Task to handle button presses
#[embassy_executor::task]
async fn button_task(
    mut button_pin: Input<'static, GpioPin<10>>,
    sender: Sender<'static, NoopRawMutex, ButtonMessage, 1>,
) {
    loop {
        button_pin.wait_for_falling_edge().await;
        sender.send(ButtonMessage::ButtonPressed).await;
        println!("Button pressed, message sent.");
        Timer::after(Duration::from_millis(400)).await; // Debounce delay
    }
}

// Task to handle WiFi connection
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
            controller.start().await.unwrap();
        }
        println!("WiFi connection attempt...");

        match controller.connect().await {
            Ok(_) => {
                println!("Wifi connected!");
                if let Ok(configuration) = controller.get_configuration() {
                    println!("Controller configuration: {:?}", configuration);
                }
                if let Ok(capabilities) = controller.get_capabilities() {
                    println!("Controller capabilities: {:?}", capabilities);
                }
                println!("Checking initial config_v4...");
                if let Some(config) = stack.config_v4() {
                    println!("Initial config_v4 found: {:?}", config);
                } else {
                    println!("No initial config_v4 found, will wait for IP address...");
                }
                let mut retries = 0;
                loop {
                    println!("Current stack config_v4 state: {:?}", stack.config_v4());
                    if let Some(config) = stack.config_v4() {
                        println!("Got IP: {}", config.address);
                        sender.send(WifiMessage::WifiConnected).await;
                        break;
                    } else {
                        println!("IP connection attempt -- retry {}", retries);
                        retries += 1;
                        if retries > 20 {
                            println!("Failed to get IP address after {} retries. Restarting WiFi connection...", retries);
                            controller.stop().await.unwrap();
                            break;
                        }
                    }
                    Timer::after(Duration::from_millis(2000)).await;
                }
            }
            Err(e) => {
                println!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await;
            }
        }
    }
}

// Task to run the network stack
#[embassy_executor::task]
async fn net_task(stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>) {
    stack.run().await
}

// Task to fetch update frames from the server
#[embassy_executor::task]
async fn fetch_update_frames(
    spawner: Spawner,
    receiver: Receiver<'static, NoopRawMutex, WifiMessage, 1>,
    stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>,
    sender: Sender<'static, NoopRawMutex, FetchMessage, 1>,
) {
    let dns_socket = DnsSocket::new(stack);
    let hostname = "api.openf1.org";

    loop {
        match receiver.receive().await {
            WifiMessage::WifiConnected => {
                println!("Fetching update frames started...");
                match dns_socket.query(hostname, DnsQueryType::A).await {
                    Ok(ip_addresses) => {
                        if let Some(IpAddress::Ipv4(ip_address)) = ip_addresses.get(0) {
                            let remote_endpoint = (*ip_address, 80);
                            let mut rx_buffer = [0; 4096];
                            let mut tx_buffer = [0; 4096];
                            let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
                            socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));
                            println!("Connecting to {}...", hostname);
                            match socket.connect(remote_endpoint).await {
                                Ok(_) => {
                                    println!("Connected to {}..", hostname);
                                    spawner.spawn(fetch_data(stack, *ip_address, sender)).unwrap();
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

// Task to fetch data from the server
#[embassy_executor::task]
async fn fetch_data(
    stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>,
    ip_address: Ipv4Address,
    sender: Sender<'static, NoopRawMutex, FetchMessage, 1>,
) {
    let session_key = "9149";
    let driver_numbers = [
        1, 2, 4, 10, 11, 14, 16, 18, 20, 22, 23, 24, 27, 31, 40, 44, 55, 63, 77, 81,
    ];
    let start_time = "2023-08-27T12:58:56.234";
    let end_time = "2023-08-27T12:58:57.154";

    let mut all_data = Vec::<FetchedData, 64>::new();

    for &driver_number in &driver_numbers {
        let mut url: String<256> = String::new();
        url.push_str("GET /v1/location?session_key=")
            .unwrap();
        url.push_str(session_key).unwrap();
        url.push_str("&driver_number=").unwrap();
        push_u32(&mut url, driver_number).unwrap();
        url.push_str("&date%3E").unwrap(); // Encoding for '>'
        url.push_str(start_time).unwrap();
        url.push_str("&date%3C").unwrap(); // Encoding for '<'
        url.push_str(end_time).unwrap();
        url.push_str(" HTTP/1.1\r\nHost: api.openf1.org\r\nConnection: close\r\n\r\n").unwrap();

        println!("Sending request: {}", url);

        let mut retries = 0;
        loop {
            let mut rx_buffer = [0; 4096];
            let mut tx_buffer = [0; 4096];
            let remote_endpoint = (ip_address, 80); // Use the resolved IP address here
            let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
            socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

            match socket.connect(remote_endpoint).await {
                Ok(_) => {
                    println!("Connected to api.openf1.org");
                    socket.write_all(url.as_bytes()).await.unwrap();

                    let mut response = [0u8; 4096];  // Increased buffer size to handle larger responses
                    let mut total_read = 0;

                    loop {
                        match socket.read(&mut response[total_read..]).await {
                            Ok(0) => break, // Connection closed
                            Ok(n) => total_read += n,
                            Err(e) => {
                                println!("Error reading response: {:?}", e);
                                break;
                            }
                        }
                    }

                    println!("Raw response: {:?}", &response[..total_read]);

                    if total_read > 0 {
                        if response.starts_with(b"HTTP/1.1 200 OK") || response.starts_with(b"HTTP/1.0 200 OK") {
                            if let Some(body_start) = find_http_body(&response[..total_read]) {
                                let body = &response[body_start..total_read];
                                println!("Body: {:?}", body);

                                let data: Result<Vec<FetchedData, 32>, _> = from_slice(body).map(|(d, _)| d);
                                match data {
                                    Ok(data) => {
                                        println!("Parsed data: {:?}", data);
                                        for item in data {
                                            all_data.push(item).unwrap();
                                        }
                                    }
                                    Err(e) => {
                                        println!("Failed to parse JSON: {:?}", e);
                                    }
                                }
                            } else {
                                println!("Failed to find body in HTTP response.");
                            }
                        } else {
                            // Log the non-200 HTTP response and move on
                            println!("Non-200 HTTP response received");
                        }
                    } else {
                        println!("Empty response received.");
                    }
                    break;
                }
                Err(e) => {
                    retries += 1;
                    if retries > 5 {
                        println!("Failed to connect after 5 retries: {:?}", e);
                        break;
                    }
                    println!("Connect error: {:?}. Retrying... ({}/5)", e, retries);
                    Timer::after(Duration::from_millis(2000)).await;
                }
            }
        }
    }

    sender.send(FetchMessage::FetchedData(all_data)).await; 
}

// Helper function to convert u32 to string and append to buffer
fn push_u32(buf: &mut String<256>, num: u32) -> Result<(), ()> {
    let mut temp: String<10> = String::new();
    write!(temp, "{}", num).unwrap();
    buf.push_str(&temp).unwrap();
    Ok(())
}

// Helper function to find the start of the HTTP body
fn find_http_body(response: &[u8]) -> Option<usize> {
    let header_end = b"\r\n\r\n";
    response
        .windows(header_end.len())
        .position(|window| window == header_end)
        .map(|pos| pos + header_end.len())
}

// Task to store fetched data
#[embassy_executor::task]
async fn store_data(receiver: Receiver<'static, NoopRawMutex, FetchMessage, 1>) {
    let mut data_to_be_visualized: Option<Vec<FetchedData, 64>> = None;

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
