#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod data;
mod driver_info;
mod hd108;
mod simple_rng;

use data::VISUALIZATION_DATA;
use driver_info::DRIVERS;

use portable_atomic::{AtomicBool, Ordering};

use chrono::{Datelike, Duration as ChronoDuration, NaiveDateTime, Timelike};
use core::fmt::Write as FmtWrite;
use core::str;
use embassy_executor::Spawner;
use embassy_net::{
    Config, IpAddress, Ipv4Address, Ipv4Cidr, Stack, StackResources, StaticConfigV4,
};
use embassy_net::tcp::TcpSocket;
use embassy_net::dns::{DnsSocket, DnsQueryType};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};
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
use grounded::uninit::GroundedArrayCell;
use hd108::HD108;
use heapless::String as HeaplessString;
use heapless::Vec as HeaplessVec;
use panic_halt as _;
use rustls::{ClientConfig, ClientConnection, Stream};
use serde::{Deserialize, Serialize};
use static_cell::StaticCell;
use core::convert::TryInto;

type HeaplessVec08<T, const N: usize> = HeaplessVec<T, N>;

macro_rules! mk_static {
    ($t:path, $val:expr) => {{
        static STATIC_CELL: StaticCell<$t> = StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.init($val);
        x
    }};
}

// CONFIG

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");

const STATIC_IP: &str = "192.168.1.100";
const SUBNET_MASK: &str = "255.255.255.0";
const GATEWAY: &str = "192.168.1.1";
const DNS_SERVER: &str = "8.8.8.8";

// Size of DEC in bytes
const DEC_SIZE: usize = 3382548;

// Total MCU flash size in bytes
const MCU_FLASH_SIZE: usize = 4194304;

// Flag for dynamic time updates
static DYNAMIC_TIME_UPDATES: bool = true;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FetchedData {
    date: HeaplessString<32>,
    driver_number: u32,
    meeting_key: u32,
    session_key: u32,
    x: i32,
    y: i32,
    z: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FetchedDataWrapper {
    data: HeaplessVec08<FetchedData, 100>, // Use HeaplessVec08 for handling dynamic size within fixed limit
}

#[derive(Debug, Clone, PartialEq)]
struct RaceTimes {
    pub start_time: chrono::NaiveTime,
    pub end_time: chrono::NaiveTime,
}

enum ButtonMessage {
    ButtonPressed,
}

enum ConnectionMessage {
    WifiInitialized,
    IpAddressAcquired,
    SocketConnected,
    Disconnected,
}

enum FetchMessage {
    FetchedData(HeaplessVec08<FetchedData, 64>), // Dynamically sized vector for the fetched data
}

static BUTTON_CHANNEL: StaticCell<Channel<NoopRawMutex, ButtonMessage, 1>> = StaticCell::new();
static CONNECTION_CHANNEL: StaticCell<Channel<NoopRawMutex, ConnectionMessage, 1>> = StaticCell::new();
static FETCH_CHANNEL: StaticCell<Channel<NoopRawMutex, FetchMessage, 1>> = StaticCell::new();

static SOCKET_RX_BUFFER: StaticCell<[u8; 4096]> = StaticCell::new();
static SOCKET_TX_BUFFER: StaticCell<[u8; 4096]> = StaticCell::new();

// Define a static memory pool using GroundedArrayCell
static MEMORY_POOL: GroundedArrayCell<u8, 4096> = GroundedArrayCell::const_init();
static FETCHED_DATA_SIZE: StaticCell<usize> = StaticCell::new();
static MEMORY_FULL: AtomicBool = AtomicBool::new(false);

#[embassy_executor::main]
async fn main(spawner: Spawner) {
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
    let connection_channel = CONNECTION_CHANNEL.init(Channel::new());
    let fetch_channel = FETCH_CHANNEL.init(Channel::new());

    // Initialize FETCHED_DATA_SIZE
    FETCHED_DATA_SIZE.init(0);

    // Spawn the button task with ownership of the button pin and the sender
    if let Err(e) = spawner.spawn(button_task(button_pin, button_channel.sender())) {
        println!("Failed to spawn button_task: {:?}", e);
    }

    // Spawn the run_race_task with the receiver
    if let Err(e) = spawner.spawn(run_race_task(hd108, button_channel.receiver())) {
        println!("Failed to spawn run_race_task: {:?}", e);
    }

    if let Err(e) = spawner.spawn(store_data(fetch_channel.receiver())) {
        println!("Failed to spawn store_data: {:?}", e);
    }

    // Wifi
    let timer = esp_hal::timer::systimer::SystemTimer::new(peripherals.SYSTIMER).alarm0;

    println!("Initializing WiFi...");

    let rng = esp_hal::rng::Rng::new(peripherals.RNG);

    match esp_wifi::initialize(
        esp_wifi::EspWifiInitFor::Wifi,
        timer,
        rng,
        peripherals.RADIO_CLK,
        &clocks,
    ) {
        Ok(init_wifi) => {
            println!("WiFi initialized...");
            let wifi = peripherals.WIFI;
            match esp_wifi::wifi::new_with_mode(&init_wifi, wifi, esp_wifi::wifi::WifiStaDevice) {
                Ok((wifi_interface, controller)) => {
                    println!("WiFi controller and interface created...");

                    let static_ip: Ipv4Address = STATIC_IP.parse().unwrap();
                    let subnet_mask: u8 = 24; // For 255.255.255.0
                    let gateway: Ipv4Address = GATEWAY.parse().unwrap();
                    let dns_server: Ipv4Address = DNS_SERVER.parse().unwrap();

                    let config = Config::ipv4_static(StaticConfigV4 {
                        address: Ipv4Cidr::new(static_ip, subnet_mask),
                        gateway: Some(gateway),
                        dns_servers: HeaplessVec08::from_slice(&[dns_server]).unwrap(),
                    });
                    let seed = 1234;

                    // Init network stack
                    let stack = &*mk_static!(
                        Stack<esp_wifi::wifi::WifiDevice<'_, esp_wifi::wifi::WifiStaDevice>>,
                        Stack::new(
                            wifi_interface,
                            config,
                            mk_static!(StackResources<3>, StackResources::<3>::new()),
                            seed
                        )
                    );

                    if let Err(e) = spawner.spawn(wifi_connection(
                        controller,
                        stack,
                        connection_channel.receiver(),
                        connection_channel.sender(),
                    )) {
                        println!("Failed to spawn wifi_connection: {:?}", e);
                    } else {
                        println!("WiFi Connection spawned...");
                        connection_channel
                            .sender()
                            .send(ConnectionMessage::WifiInitialized)
                            .await;
                        println!("WifiInitialized message sent successfully");
                    }

                    println!("Starting network stack...");
                    if let Err(e) = spawner.spawn(run_network_stack(stack)) {
                        println!("Failed to spawn run_network_stack: {:?}", e);
                    } else {
                        println!("Network stack task spawned...");
                    }

                    // Add a delay to ensure the network stack has time to initialize
                    Timer::after(Duration::from_secs(2)).await;

                    if let Err(e) = spawner.spawn(fetch_update_frames(
                        connection_channel.receiver(),
                        stack,
                        fetch_channel.sender(),
                        connection_channel.sender(),
                        spawner,
                    )) {
                        println!("Failed to spawn fetch_update_frames: {:?}", e);
                    } else {
                        println!("Fetch Update Frames Spawned...");
                    }
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

fn parse_iso8601_timestamp(timestamp: &str) -> Result<NaiveDateTime, chrono::ParseError> {
    let cleaned_timestamp = if timestamp.ends_with('Z') {
        &timestamp[..timestamp.len() - 1]
    } else {
        timestamp
    };

    let naive_datetime = NaiveDateTime::parse_from_str(cleaned_timestamp, "%Y-%m-%dT%H:%M:%S%.f")?;

    Ok(naive_datetime)
}

fn add_milliseconds_to_naive_datetime(datetime: NaiveDateTime, milliseconds: i64) -> NaiveDateTime {
    let duration = ChronoDuration::milliseconds(milliseconds);
    datetime + duration
}

fn naive_datetime_to_iso8601(datetime: NaiveDateTime) -> HeaplessString<32> {
    let year = datetime.year();
    let month = datetime.month();
    let day = datetime.day();
    let hour = datetime.hour();
    let minute = datetime.minute();
    let second = datetime.second();
    let millisecond = datetime.and_utc().timestamp_subsec_millis();

    let mut iso8601 = HeaplessString::<32>::new();

    let _ = write!(&mut iso8601, "{:04}", year);
    iso8601.push('-').unwrap();

    let _ = write!(&mut iso8601, "{:02}", month);
    iso8601.push('-').unwrap();

    let _ = write!(&mut iso8601, "{:02}", day);
    iso8601.push('T').unwrap();

    let _ = write!(&mut iso8601, "{:02}", hour);
    iso8601.push(':').unwrap();

    let _ = write!(&mut iso8601, "{:02}", minute);
    iso8601.push(':').unwrap();

    let _ = write!(&mut iso8601, "{:02}", second);
    iso8601.push('.').unwrap();

    let _ = write!(&mut iso8601, "{:03}", millisecond);
    iso8601.push('Z').unwrap();

    iso8601
}

fn monitor_memory_task() -> usize {
    let total_flashed_memory = DEC_SIZE;
    let fetched_data_size = *FETCHED_DATA_SIZE.get().unwrap();
    let remaining_memory = MCU_FLASH_SIZE - total_flashed_memory - fetched_data_size;

    println!("Total MCU memory: {} bytes", MCU_FLASH_SIZE);
    println!("Total binary size: {} bytes", total_flashed_memory);
    println!("Fetched data memory used: {} bytes", fetched_data_size);
    println!("Remaining memory: {} bytes", remaining_memory);

    if remaining_memory <= 0 {
        MEMORY_FULL.store(true, Ordering::SeqCst);
    } else {
        MEMORY_FULL.store(false, Ordering::SeqCst);
    }

    remaining_memory
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
                for frame in &data::VISUALIZATION_DATA.frames {
                    let mut led_updates: HeaplessVec08<(usize, u8, u8, u8), 20> = HeaplessVec::new();

                    for driver_data in frame.drivers.iter().flatten() {
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

                    hd108.set_leds(&led_updates).await.unwrap();

                    Timer::after(Duration::from_millis(
                        data::VISUALIZATION_DATA.update_rate_ms as u64,
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

#[embassy_executor::task]
async fn button_task(
    mut button_pin: Input<'static, GpioPin<10>>,
    sender: Sender<'static, NoopRawMutex, ButtonMessage, 1>,
) {
    loop {
        button_pin.wait_for_falling_edge().await;
        sender.send(ButtonMessage::ButtonPressed).await;
        println!("Button pressed, message sent.");
        Timer::after(Duration::from_millis(400)).await;
    }
}

#[embassy_executor::task]
async fn wifi_connection(
    mut controller: esp_wifi::wifi::WifiController<'static>,
    stack: &'static Stack<esp_wifi::wifi::WifiDevice<'static, esp_wifi::wifi::WifiStaDevice>>,
    receiver: Receiver<'static, NoopRawMutex, ConnectionMessage, 1>,
    sender: Sender<'static, NoopRawMutex, ConnectionMessage, 1>,
) {
    println!("Starting wifi connection task");
    match receiver.receive().await {
        ConnectionMessage::WifiInitialized => {
            println!("Wifi initialized message received, proceeding...");

            let mut ssid: HeaplessString<32> = HeaplessString::new();
            ssid.push_str(SSID).unwrap();

            let mut password: HeaplessString<64> = HeaplessString::new();
            password.push_str(PASSWORD).unwrap();

            println!("Setting controller configuration...");
            controller
                .set_configuration(&esp_wifi::wifi::Configuration::Client(esp_wifi::wifi::ClientConfiguration {
                    ssid,
                    password,
                    ..Default::default()
                }))
                .unwrap();

            controller.start().await.unwrap();

            let mut retries = 0;
            const MAX_RETRIES: u32 = 5;

            while retries < MAX_RETRIES {
                println!("Retries count: {}", retries);
                match controller.connect().await {
                    Ok(_) => {
                        println!("WiFi connected successfully.");
                        break;
                    }
                    Err(e) => {
                        retries += 1;
                        println!(
                            "Failed to connect to WiFi: {:?}. Retrying {}/{}",
                            e, retries, MAX_RETRIES
                        );
                        Timer::after(Duration::from_secs(2)).await;
                    }
                }
            }

            if retries >= MAX_RETRIES {
                println!("Failed to connect to WiFi after {} retries.", MAX_RETRIES);
                sender.send(ConnectionMessage::Disconnected).await;
            }

            let mut retries = 0;
            while retries < 20 {
                if stack.is_link_up() {
                    if let Some(config) = stack.config_v4() {
                        println!("Got IP: {}", config.address);
                        sender.send(ConnectionMessage::IpAddressAcquired).await;
                        break;
                    }
                }
                retries += 1;
                Timer::after(Duration::from_secs(1)).await;
            }

            if retries >= 20 {
                println!("Failed to acquire IP address.");
                sender.send(ConnectionMessage::Disconnected).await;
            }
        }
        _ => {}
    }
}

#[embassy_executor::task]
async fn run_network_stack(stack: &'static Stack<esp_wifi::wifi::WifiDevice<'static, esp_wifi::wifi::WifiStaDevice>>) {
    println!("Running network stack...");
    stack.run().await;
    println!("Network stack exited...");
}

#[embassy_executor::task]
async fn fetch_update_frames(
    connection_receiver: Receiver<'static, NoopRawMutex, ConnectionMessage, 1>,
    stack: &'static Stack<esp_wifi::wifi::WifiDevice<'static, esp_wifi::wifi::WifiStaDevice>>,
    fetch_sender: Sender<'static, NoopRawMutex, FetchMessage, 1>,
    connection_sender: Sender<'static, NoopRawMutex, ConnectionMessage, 1>,
    spawner: Spawner,
) {
    match connection_receiver.receive().await {
        ConnectionMessage::IpAddressAcquired => {
            println!("IP Address acquired.");

            if let Err(e) = spawner.spawn(dns_query_task(
                stack,
                fetch_sender,
                connection_sender,
                spawner,
            )) {
                println!("Failed to spawn dns_query_task: {:?}", e);
            }
        }
        _ => {
            println!("Other connection message received");
        }
    }
}

#[embassy_executor::task]
async fn dns_query_task(
    stack: &'static Stack<esp_wifi::wifi::WifiDevice<'static, esp_wifi::wifi::WifiStaDevice>>,
    fetch_sender: Sender<'static, NoopRawMutex, FetchMessage, 1>,
    connection_sender: Sender<'static, NoopRawMutex, ConnectionMessage, 1>,
    spawner: Spawner,
) {
    let dns_socket = DnsSocket::new(stack);
    let hostname = "api.openf1.org";

    println!("Querying DNS for {}", hostname);
    match dns_socket.query(hostname, DnsQueryType::A).await {
        Ok(ip_addresses) => {
            if let Some(ip_address) = ip_addresses.get(0) {
                let IpAddress::Ipv4(ipv4_address) = ip_address;
                let remote_endpoint = (*ipv4_address, 443);

                if let Err(e) = spawner.spawn(fetch_data_loop(
                    stack,
                    remote_endpoint,
                    fetch_sender,
                )) {
                    println!("Failed to spawn fetch_data_loop: {:?}", e);
                }
            } else {
                println!("No IP addresses found for {}", hostname);
            }
        }
        Err(e) => {
            println!("Failed to query DNS: {:?}", e);
        }
    }
}

#[embassy_executor::task]
async fn fetch_data_loop(
    stack: &'static Stack<esp_wifi::wifi::WifiDevice<'static, esp_wifi::wifi::WifiStaDevice>>,
    remote_endpoint: (Ipv4Address, u16),
    fetch_sender: Sender<'static, NoopRawMutex, FetchMessage, 1>,
) {
    let start_time_str = "2023-08-27T12:58:56.234";
    let end_time_str = "2023-08-27T12:58:57.154";
    let mut start_time = parse_iso8601_timestamp(start_time_str).unwrap();
    let mut end_time = parse_iso8601_timestamp(end_time_str).unwrap();

    let mut config = ClientConfig::new();
    config.root_store = rustls::RootCertStore::empty(); // Ensure the root certificate store is set properly
    let client = ClientConnection::new(Arc::new(config), "api.openf1.org".try_into().unwrap()).unwrap();

    loop {
        match fetch_data_https(stack, client.clone(), start_time, end_time).await {
            Ok(data) => {
                fetch_sender.send(FetchMessage::FetchedData(data)).await;
            }
            Err(e) => {
                println!("Failed to fetch data: {:?}", e);
                Timer::after(Duration::from_secs(5)).await;
            }
        }

        Timer::after(Duration::from_millis(1150)).await;
    }
}

async fn fetch_data_https(
    stack: &Stack<esp_wifi::wifi::WifiDevice<'_, esp_wifi::wifi::WifiStaDevice>>,
    mut client: ClientConnection,
    start_time: NaiveDateTime,
    end_time: NaiveDateTime,
) -> Result<HeaplessVec08<FetchedData, 64>, ()> {
    let session_key = "9149";
    let driver_number = 1;

    let url = core::format!(
        "https://api.openf1.org/v1/location?session_key={}&driver_number={}&date>{}&date<{}",
        session_key,
        driver_number,
        naive_datetime_to_iso8601(start_time),
        naive_datetime_to_iso8601(end_time)
    );

    println!("Sending request to: {}", url);

    let mut socket = TcpSocket::new(stack, SOCKET_RX_BUFFER.get().unwrap(), SOCKET_TX_BUFFER.get().unwrap());
    socket.set_timeout(Some(Duration::from_secs(10)));
    socket.connect((stack, remote_endpoint)).await.unwrap();
    let mut stream = Stream::new(&mut client, &mut socket);

    let request = core::format!(
        "GET /v1/location?session_key={}&driver_number={}&date>{}&date<{} HTTP/1.1\r\nHost: api.openf1.org\r\nConnection: close\r\n\r\n",
        session_key,
        driver_number,
        naive_datetime_to_iso8601(start_time),
        naive_datetime_to_iso8601(end_time)
    );

    stream.write_all(request.as_bytes()).await.unwrap();

    let mut response = [0u8; 4096];
    let n = stream.read(&mut response).await.unwrap();
    let response_str = str::from_utf8(&response[..n]).unwrap();

    println!("Received response: {}", response_str);

    let wrapper: FetchedDataWrapper = serde_json::from_str(response_str).unwrap();

    let mut all_data = HeaplessVec::<FetchedData, 64>::new();
    for item in &wrapper.data {
        all_data.push(item.clone()).unwrap();
    }

    Ok(all_data)
}

#[embassy_executor::task]
async fn store_data(receiver: Receiver<'static, NoopRawMutex, FetchMessage, 1>) {
    loop {
        match receiver.receive().await {
            FetchMessage::FetchedData(data) => {
                println!("Received data: {:?}", data);

                monitor_memory_task();
            }
        }
    }
}
