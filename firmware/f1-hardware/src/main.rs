#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod data;
mod driver_info;
mod hd108;
mod simple_rng; // Assuming your custom RNG implementation is in this module
use data::VISUALIZATION_DATA;
use driver_info::DRIVERS;

use chrono::{Datelike, Duration as ChronoDuration, NaiveDateTime, Timelike};
use core::fmt::Write as FmtWrite;
use core::ptr::addr_of_mut;
use core::str;
use core::sync::atomic::{AtomicBool, Ordering};
use embassy_executor::Spawner;
use embassy_net::dns::{DnsQueryType, DnsSocket};
use embassy_net::{
    Config, IpAddress, Ipv4Address, Ipv4Cidr, Stack, StackResources, StaticConfigV4,
};
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
use grounded::uninit::GroundedCell;
use hd108::HD108;
use heapless08::{String as Heapless08String, Vec as Heapless08Vec};
use panic_halt as _;
use postcard;
use serde::{Deserialize, Serialize};
use serde_json_core::from_slice;
use static_cell::StaticCell;

// Importing necessary TLS modules
use embassy_net::tcp::{ConnectError, TcpSocket};
use embedded_io_async::{Read, Write};
use embedded_tls::{Aes128GcmSha256, Certificate, NoVerify, TlsConfig, TlsConnection, TlsContext};
use esp_hal::rng::Rng;
use esp_wifi::{
    initialize,
    wifi::{ClientConfiguration, Configuration, WifiController, WifiDevice, WifiStaDevice},
    EspWifiInitFor,
};

use simple_rng::SimpleRng; // Import your custom RNG

type HeaplessVec08<T, const N: usize> = Heapless08Vec<T, N>;

macro_rules! mk_static {
    ($t:path, $val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
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
    date: Heapless08String<32>,
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
    FetchedData(Heapless08Vec<FetchedData, 64>), // Dynamically sized vector for the fetched data
}

static BUTTON_CHANNEL: StaticCell<Channel<NoopRawMutex, ButtonMessage, 1>> = StaticCell::new();
static CONNECTION_CHANNEL: StaticCell<Channel<NoopRawMutex, ConnectionMessage, 1>> =
    StaticCell::new();
static FETCH_CHANNEL: StaticCell<Channel<NoopRawMutex, FetchMessage, 1>> = StaticCell::new();

static SOCKET_RX_BUFFER: StaticCell<[u8; 4096]> = StaticCell::new();
static SOCKET_TX_BUFFER: StaticCell<[u8; 4096]> = StaticCell::new();

// Define a static memory pool using GroundedArrayCell
static MEMORY_POOL: GroundedArrayCell<u8, 4096> = GroundedArrayCell::const_init();
static FETCHED_DATA_SIZE: GroundedCell<usize> = GroundedCell::uninit();
static MEMORY_FULL: AtomicBool = AtomicBool::new(false);

#[main]
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
    unsafe {
        *FETCHED_DATA_SIZE.get() = 0;
    }

    // Spawn the button task with ownership of the button pin and the sender
    if let Err(e) = spawner
        .spawn(button_task(button_pin, button_channel.sender().into()))
    {
        println!("Failed to spawn button_task: {:?}", e);
    }

    // Spawn the run_race_task with the receiver
    if let Err(e) = spawner
        .spawn(run_race_task(hd108, button_channel.receiver().into()))
    {
        println!("Failed to spawn run_race_task: {:?}", e);
    }

    if let Err(e) = spawner
        .spawn(store_data(fetch_channel.receiver().into()))
    {
        println!("Failed to spawn store_data: {:?}", e);
    }

    // Wifi
    let timer = esp_hal::timer::systimer::SystemTimer::new(peripherals.SYSTIMER).alarm0;

    println!("Initializing WiFi...");

    let rng = Rng::new(peripherals.RNG); // Correct instantiation of Rng

    match initialize(
        EspWifiInitFor::Wifi,
        timer,
        rng,
        peripherals.RADIO_CLK,
        &clocks,
    ) {
        Ok(init_wifi) => {
            println!("WiFi initialized...");
            let wifi = peripherals.WIFI;
            match esp_wifi::wifi::new_with_mode(&init_wifi, wifi, WifiStaDevice) {
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

                    if let Err(e) = spawner
                        .spawn(wifi_connection(
                            controller,
                            stack,
                            connection_channel.receiver().into(),
                            connection_channel.sender().into(),
                        ))
                    {
                        println!("Failed to spawn wifi_connection: {:?}", e);
                    } else {
                        println!("WiFi Connection spawned...");
                        // Send WifiInitialized message after tasks are spawned
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

                    if let Err(e) = spawner
                        .spawn(fetch_update_frames(
                            connection_channel.receiver().into(),
                            stack,
                            fetch_channel.sender().into(),
                            connection_channel.sender().into(),
                            spawner,
                        ))
                    {
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
    // Remove the trailing 'Z' if present
    let cleaned_timestamp = if timestamp.ends_with('Z') {
        &timestamp[..timestamp.len() - 1]
    } else {
        timestamp
    };

    // Parse the timestamp
    let naive_datetime = NaiveDateTime::parse_from_str(cleaned_timestamp, "%Y-%m-%dT%H:%M:%S%.f")?;

    Ok(naive_datetime)
}

fn add_milliseconds_to_naive_datetime(datetime: NaiveDateTime, milliseconds: i64) -> NaiveDateTime {
    // Create a Duration from the milliseconds
    let duration = ChronoDuration::milliseconds(milliseconds);
    // Add the duration to the NaiveDateTime
    datetime + duration
}

fn naive_datetime_to_iso8601(datetime: NaiveDateTime) -> Heapless08String<32> {
    let year = datetime.year();
    let month = datetime.month();
    let day = datetime.day();
    let hour = datetime.hour();
    let minute = datetime.minute();
    let second = datetime.second();
    let millisecond = datetime.and_utc().timestamp_subsec_millis();

    // Create a new heapless string with a capacity of 32 bytes
    let mut iso8601 = Heapless08String::<32>::new();

    // Manually construct the ISO 8601 string
    // Write year
    let _ = write!(&mut iso8601, "{:04}", year);
    iso8601.push('-').unwrap();

    // Write month
    let _ = write!(&mut iso8601, "{:02}", month);
    iso8601.push('-').unwrap();

    // Write day
    let _ = write!(&mut iso8601, "{:02}", day);
    iso8601.push('T').unwrap();

    // Write hour
    let _ = write!(&mut iso8601, "{:02}", hour);
    iso8601.push(':').unwrap();

    // Write minute
    let _ = write!(&mut iso8601, "{:02}", minute);
    iso8601.push(':').unwrap();

    // Write second
    let _ = write!(&mut iso8601, "{:02}", second);
    iso8601.push('.').unwrap();

    // Write millisecond
    let _ = write!(&mut iso8601, "{:03}", millisecond);
    iso8601.push('Z').unwrap();

    iso8601
}

fn monitor_memory_task() -> usize {
    let total_flashed_memory = DEC_SIZE;
    let fetched_data_size = unsafe { *FETCHED_DATA_SIZE.get() };
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
                // Iterate through each frame in the visualization data
                for frame in &data::VISUALIZATION_DATA.frames {
                    // Collect the LED updates for the current frame
                    let mut led_updates: Heapless08Vec<(usize, u8, u8, u8), 20> =
                        Heapless08Vec::new();

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
    receiver: Receiver<'static, NoopRawMutex, ConnectionMessage, 1>,
    sender: Sender<'static, NoopRawMutex, ConnectionMessage, 1>,
) {
    println!("Starting wifi connection task");
    match receiver.receive().await {
        ConnectionMessage::WifiInitialized => {
            println!("Wifi initialized message received, proceeding...");

            let mut ssid: Heapless08String<32> = Heapless08String::new();
            ssid.push_str(SSID).unwrap();

            let mut password: Heapless08String<64> = Heapless08String::new();
            password.push_str(PASSWORD).unwrap();

            println!("Setting controller configuration...");
            controller
                .set_configuration(&Configuration::Client(ClientConfiguration {
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

            // Wait for IP address assignment
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
async fn run_network_stack(stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>) {
    println!("Running network stack...");
    stack.run().await;
    println!("Network stack exited...");
}

#[embassy_executor::task]
async fn fetch_update_frames(
    connection_receiver: Receiver<'static, NoopRawMutex, ConnectionMessage, 1>,
    stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>,
    fetch_sender: Sender<'static, NoopRawMutex, FetchMessage, 1>,
    _connection_sender: Sender<'static, NoopRawMutex, ConnectionMessage, 1>,
    spawner: Spawner,
) {
    match connection_receiver.receive().await {
        ConnectionMessage::IpAddressAcquired => {
            // Handle the case where the IP address is acquired
            println!("IP Address acquired.");

            // Directly use the IP address for localhost
            let remote_endpoint = (Ipv4Address::new(127, 0, 0, 1), 443);

            // Initialize static buffers
            let rx_buffer = SOCKET_RX_BUFFER.init([0; 4096]);
            let tx_buffer = SOCKET_TX_BUFFER.init([0; 4096]);

            static mut SOCKET: Option<TcpSocket<'static>> = None;
            unsafe {
                if SOCKET.is_none() {
                    SOCKET = Some(TcpSocket::new(stack, rx_buffer, tx_buffer));
                }

                if let Some(socket) = SOCKET.as_mut() {
                    let socket_ptr = addr_of_mut!(*socket);

                    if let Err(e) = spawner
                        .spawn(fetch_data_loop(
                            stack,
                            remote_endpoint,
                            socket_ptr,
                            fetch_sender,
                        ))
                    {
                        println!("Failed to spawn fetch_data_loop: {:?}", e);
                    }
                } else {
                    // Handle the case where the socket is not initialized
                    println!("Socket not initialized");
                }
            }
        }
        _ => {
            // Handle all other cases
            println!("Other connection message received");
        }
    }
}

#[embassy_executor::task]
async fn dns_query_task(
    stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>,
    fetch_sender: Sender<'static, NoopRawMutex, FetchMessage, 1>,
    _connection_sender: Sender<'static, NoopRawMutex, ConnectionMessage, 1>,
    spawner: Spawner,
) {
    let dns_socket = DnsSocket::new(stack);
    let hostname = "localhost";

    println!("Querying DNS for {}", hostname);
    match dns_socket.query(hostname, DnsQueryType::A).await {
        Ok(ip_addresses) => {
            if let Some(ip_address) = ip_addresses.get(0) {
                let IpAddress::Ipv4(ipv4_address) = ip_address;
                let remote_endpoint = (*ipv4_address, 443); // Using port 443 for HTTPS

                // Initialize static buffers
                let rx_buffer = SOCKET_RX_BUFFER.init([0; 4096]);
                let tx_buffer = SOCKET_TX_BUFFER.init([0; 4096]);

                static mut SOCKET: Option<TcpSocket<'static>> = None;
                unsafe {
                    if SOCKET.is_none() {
                        SOCKET = Some(TcpSocket::new(stack, rx_buffer, tx_buffer));
                    }

                    if let Some(socket) = SOCKET.as_mut() {
                        let socket_ptr = addr_of_mut!(*socket);

                        if let Err(e) = spawner
                            .spawn(fetch_data_loop(
                                stack,
                                remote_endpoint,
                                socket_ptr,
                                fetch_sender,
                            ))
                        {
                            println!("Failed to spawn fetch_data_loop: {:?}", e);
                        }
                    } else {
                        // Handle the case where the socket is not initialized
                        println!("Socket not initialized");
                    }
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
    _stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>,
    _remote_endpoint: (Ipv4Address, u16),
    socket_ptr: *mut TcpSocket<'static>,
    fetch_sender: Sender<'static, NoopRawMutex, FetchMessage, 1>,
) {
    loop {
        let mut attempt = 0;
        const MAX_ATTEMPTS: usize = 5;

        // Initialize the TLS session once and reuse it
        let mut tls_initialized = false;

        match fetch_data_https(socket_ptr, fetch_sender, &mut tls_initialized).await {
            Ok(_) => {
                println!("Data fetched successfully.");
            }
            Err(e) => {
                println!("Failed to fetch data: {:?}", e);
                if attempt >= MAX_ATTEMPTS {
                    println!("Max attempts reached. Giving up.");
                    break;
                }
                attempt += 1;
            }
        }

        // Small delay before the next iteration
        Timer::after(Duration::from_millis(1150)).await;
    }
}

async fn socket_reset(
    stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>,
    remote_endpoint: (Ipv4Address, u16),
    socket_ptr: *mut TcpSocket<'static>,
) -> Result<(), ConnectError> {
    unsafe {
        let socket = &mut *socket_ptr;
        socket.set_timeout(Some(Duration::from_secs(10)));

        match socket.connect(remote_endpoint).await {
            Ok(_) => {
                // Successfully connected
                println!("Connected to {:?}", remote_endpoint);
                Ok(())
            }
            Err(e) => {
                // Handle the connection error
                println!("Failed to connect: {:?}", e);
                Err(e)
            }
        }
    }
}

async fn fetch_data_https(
    socket_ptr: *mut TcpSocket<'static>,
    fetch_sender: Sender<'static, NoopRawMutex, FetchMessage, 1>,
    tls_initialized: &mut bool,
) -> Result<(), embedded_tls::TlsError> {
    const BUFFER_SIZE: usize = 8192; // Increased buffer size

    println!("Initializing TLS session");

    let config = configure_tls().unwrap();

    let mut rx_buffer = [0u8; BUFFER_SIZE];
    let mut tx_buffer = [0u8; BUFFER_SIZE];

    let mut socket = unsafe { &mut *socket_ptr };
    let mut tls: TlsConnection<_, Aes128GcmSha256> =
        TlsConnection::new(&mut socket, &mut rx_buffer, &mut tx_buffer);

    let mut rng = SimpleRng::new(1234);
    let context = TlsContext::new(&config, &mut rng);

    if !*tls_initialized {
        match tls.open::<_, NoVerify>(context).await {
            Ok(_) => {
                println!("TLS session initialized successfully");
                *tls_initialized = true;
            }
            Err(e) => {
                println!("TLS session initialization failed: {:?}", e);
                return Err(e);
            }
        }
    }

    let mut url: Heapless08String<256> = Heapless08String::new();
    url.push_str("GET /mud/ HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n")
        .unwrap();

    println!("Sending request: {}", url);

    // Write the HTTP GET request to the TLS stream
    match tls.write_all(url.as_bytes()).await {
        Ok(_) => {
            println!("Request sent successfully");

            let mut response = [0u8; BUFFER_SIZE];

            // Read the response from the server
            match tls.read(&mut response).await {
                Ok(n) => {
                    if n == 0 {
                        println!("Connection closed by peer");
                        return Err(embedded_tls::TlsError::ConnectionClosed);
                    }

                    // n represents the number of bytes read
                    println!("Received response ({} bytes): {:?}", n, &response[..n]);
                    if response.starts_with(b"HTTP/1.1 200 OK")
                        || response.starts_with(b"HTTP/1.0 200 OK")
                    {
                        println!("Received OK response");

                        // Additional debug info for response content
                        if let Some(body_start) = find_http_body(&response[..n]) {
                            let body = &response[body_start..n];
                            println!("Response body: {:?}", body);
                        } else {
                            println!("Failed to find body in HTTP response.");
                        }

                        return Ok(());
                    } else {
                        println!("Non-200 HTTP response received");
                        println!("Response: {:?}", &response[..n]);
                        return Err(embedded_tls::TlsError::InternalError);
                    }
                }
                Err(e) => {
                    println!("Read error: {:?}", e);
                    return Err(e);
                }
            }
        }
        Err(e) => {
            println!("Write error: {:?}", e);
            return Err(e);
        }
    }
}

fn push_u32(buf: &mut Heapless08String<256>, num: u32) -> Result<(), ()> {
    let mut temp: Heapless08String<10> = Heapless08String::new();
    write!(temp, "{}", num).unwrap();
    buf.push_str(&temp).unwrap();
    Ok(())
}

fn find_http_body(response: &[u8]) -> Option<usize> {
    let header_end = b"\r\n\r\n";
    response
        .windows(header_end.len())
        .position(|window| window == header_end)
        .map(|pos| pos + header_end.len())
}

#[embassy_executor::task]
async fn store_data(receiver: Receiver<'static, NoopRawMutex, FetchMessage, 1>) {
    loop {
        match receiver.receive().await {
            FetchMessage::FetchedData(data) => {
                println!("Received data: {:?}", data);

                // Check remaining memory after storing data
                monitor_memory_task();

                // Perform any additional processing if necessary
            }
        }
    }
}

fn configure_tls() -> Result<TlsConfig<'static, Aes128GcmSha256>, &'static str> {
    let certificate_bytes: &[u8] =
        include_bytes!("/Applications/XAMPP/xamppfiles/etc/ssl.crt/server.crt");
    let certificate = Certificate::X509(certificate_bytes);

    let config = TlsConfig::new()
        .with_server_name("localhost")
        .with_cert(certificate);

    Ok(config)
}
