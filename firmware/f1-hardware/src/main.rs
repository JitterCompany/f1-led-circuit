#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod data;
mod driver_info;
mod hd108;
use data::VISUALIZATION_DATA;
use driver_info::DRIVERS;

use core::fmt::Write as FmtWrite;
use core::ptr::addr_of_mut;
use core::sync::atomic::{ AtomicBool, Ordering };
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
    rng::Rng,
    spi::{master::Spi, SpiMode},
    system::SystemControl,
    timer::timg::TimerGroup,
};
use esp_println::println;
use hd108::HD108;
use heapless07::{String as Heapless07String, Vec as Heapless07Vec};
use heapless08::{String as Heapless08String, Vec as Heapless08Vec};
use panic_halt as _;
use postcard::{from_bytes, to_vec};
use serde::{Deserialize, Serialize};
use serde_json_core::from_slice;
use static_cell::StaticCell;
use grounded::uninit::GroundedArrayCell;
use grounded::uninit::GroundedCell;

// Importing necessary TLS modules
use embedded_io_async::{Read, Write};
use esp_mbedtls::TlsError::MbedTlsError;
use esp_mbedtls::{asynch::Session, set_debug, Certificates, Mode, TlsVersion, X509};

// Wifi
use embassy_net::tcp::{ConnectError, TcpSocket};
use esp_wifi::{
    initialize,
    wifi::{ClientConfiguration, Configuration, WifiController, WifiDevice, WifiStaDevice},
    EspWifiInitFor,
};

type HeaplessVec08<T, const N: usize> = Heapless08Vec<T, N>;

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

const STATIC_IP: &str = "192.168.1.100";
const SUBNET_MASK: &str = "255.255.255.0";
const GATEWAY: &str = "192.168.1.1";
const DNS_SERVER: &str = "8.8.8.8";

// Size of dec
const DEC_SIZE: usize = 3263938;


// Total MCU flash size
const MCU_FLASH_SIZE: usize = 4194304;

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

impl FetchedData {
    #[allow(dead_code)]
    fn from_postcard(bytes: &[u8]) -> Result<Self, &'static str> {
        from_bytes(bytes).map_err(|_| "Failed to deserialize")
    }

    #[allow(dead_code)]
    fn to_postcard(&self) -> Result<Heapless07Vec<u8, 128>, &'static str> {
        to_vec(self).map_err(|_| "Failed to serialize")
    }
}

enum ButtonMessage {
    ButtonPressed,
}

enum ConnectionMessage {
    WifiInitialized,
    IpAddressAcquired,
    Disconnected,
    SocketConnected,
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
static SOCKET: StaticCell<TcpSocket<'static>> = StaticCell::new();

// Define a static memory pool using GroundedArrayCell
static MEMORY_POOL: GroundedArrayCell<u8, 4096> = GroundedArrayCell::const_init();
static FETCHED_DATA_SIZE: GroundedCell<usize> = GroundedCell::uninit();
static MEMORY_FULL: AtomicBool = AtomicBool::new(false);

fn get_free_memory() -> usize {
    let (ptr, len) = MEMORY_POOL.get_ptr_len();
    let used_memory = unsafe {
        // Calculate used memory by counting non-zero bytes
        let mut used = 0;
        for i in 0..len {
            if *ptr.add(i) != 0 {
                used += 1;
            }
        }
        used
    };
    len - used_memory
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
    unsafe { *FETCHED_DATA_SIZE.get() = 0; }

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

                    if let Err(e) = spawner.spawn(wifi_connection(
                        controller,
                        stack,
                        connection_channel.receiver(),
                        connection_channel.sender(),
                    )) {
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
                println!("Retriest count: {}", retries);
                match controller.connect().await {
                    Ok(_) => {
                        println!("WiFi connected successfully.");
                        //sender.send(ConnectionMessage::WifiConnected).await;
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
    connection_sender: Sender<'static, NoopRawMutex, ConnectionMessage, 1>,
    spawner: Spawner,
) {
    match connection_receiver.receive().await {
        ConnectionMessage::IpAddressAcquired => {
            // Handle the case where the IP address is acquired
            println!("IP Address acquired.");

            // Spawn the DNS query task
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
            // Handle all other cases
            println!("Other connection message received");
        }
    }
}

#[embassy_executor::task]
async fn dns_query_task(
    stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>,
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

                        // Retry mechanism for socket connection
                        const MAX_RETRIES: u8 = 5;
                        let mut retries = 0;
                        loop {
                            match socket_connect(stack, remote_endpoint, socket_ptr).await {
                                Ok(_) => {
                                    println!(
                                        "Socket connection established on attempt {}",
                                        retries + 1
                                    );
                                    connection_sender
                                        .send(ConnectionMessage::SocketConnected)
                                        .await;

                                    // Use the socket for further operations, such as establishing a TLS session
                                    if let Err(e) = spawner.spawn(fetch_data_https(
                                        socket,
                                        hostname,
                                        fetch_sender,
                                    )) {
                                        println!("Failed to spawn fetch_data_https: {:?}", e);
                                    }
                                    break;
                                }
                                Err(e) => {
                                    retries += 1;
                                    println!(
                                        "Socket connection failed (attempt {}): {:?}",
                                        retries, e
                                    );
                                    if retries >= MAX_RETRIES {
                                        println!(
                                            "Max retries reached. Aborting connection attempts."
                                        );
                                        return;
                                    }
                                    Timer::after(Duration::from_secs(2)).await; // Delay before retrying
                                }
                            }
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

async fn socket_connect(
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

#[embassy_executor::task]
async fn fetch_data_https(
    socket: &'static mut TcpSocket<'static>,
    hostname: &'static str,
    fetch_sender: Sender<'static, NoopRawMutex, FetchMessage, 1>,
) {
    const BUFFER_SIZE: usize = 2048;

    // Set debug level for TLS
    set_debug(3);

    println!("Checking TLS chain");

    // Load CA chain
    let ca_chain_result = X509::pem(concat!(include_str!("root_cert.pem"), "\0").as_bytes()).ok();
    if ca_chain_result.is_none() {
        println!("Failed to load CA chain");
        return;
    } else {
        println!("CA chain loaded");
    }

    let ca_chain = ca_chain_result.unwrap();

    println!("Initializing TLS session");

    // Initialize the TLS session
    let tls_result = esp_mbedtls::asynch::Session::<&mut TcpSocket<'_>, BUFFER_SIZE>::new(
        socket,
        hostname,
        Mode::Client,
        TlsVersion::Tls1_2,
        Certificates {
            ca_chain: Some(ca_chain),
            ..Default::default()
        },
    );

    match tls_result {
        Ok(session) => {
            println!("TLS session initialized successfully");

            println!("Starting TLS handshake");
            match session.connect().await {
                Ok(mut tls_session) => {
                    println!("TLS handshake completed successfully");

                    let session_key = "9149";
                    let driver_numbers = [
                        1, 2, 4, 10, 11, 14, 16, 18, 20, 22, 23, 24, 27, 31, 40, 44, 55, 63, 77, 81,
                    ];
                    let start_time = "2023-08-27T12:58:56.234";
                    let end_time = "2023-08-27T12:58:57.154";

                    let mut all_data = Heapless08Vec::<FetchedData, 64>::new();

                    for &driver_number in &driver_numbers {
                        let mut url: Heapless08String<256> = Heapless08String::new();
                        url.push_str("GET /v1/location?session_key=").unwrap();
                        url.push_str(session_key).unwrap();
                        url.push_str("&driver_number=").unwrap();
                        push_u32(&mut url, driver_number).unwrap();
                        url.push_str("&date%3E").unwrap(); // Encoding for '>'
                        url.push_str(start_time).unwrap();
                        url.push_str("&date%3C").unwrap(); // Encoding for '<'
                        url.push_str(end_time).unwrap();
                        url.push_str(
                            " HTTP/1.1\r\nHost: api.openf1.org\r\nConnection: keep-alive\r\n\r\n",
                        )
                        .unwrap();

                        println!("Sending request: {}", url);

                        let r = tls_session.write_all(url.as_bytes()).await;
                        if let Err(e) = r {
                            println!("write error: {:?}", e);
                            continue;
                        }

                        println!("Request sent successfully");

                        let mut response = [0u8; BUFFER_SIZE];

                        match embassy_time::with_timeout(
                            Duration::from_secs(10),
                            tls_session.read(&mut response),
                        )
                        .await
                        {
                            Ok(Ok(n)) => {
                                if n == 0 {
                                    println!("Connection closed by peer");
                                    break;
                                }

                                if response.starts_with(b"HTTP/1.1 200 OK")
                                    || response.starts_with(b"HTTP/1.0 200 OK")
                                {
                                    println!("Received OK response");

                                    if let Some(body_start) = find_http_body(&response[..n]) {
                                        let body = &response[body_start..n];
                                        //println!("Response body: {:?}", body);

                                        let data: Result<Heapless08Vec<FetchedData, 32>, _> =
                                            from_slice(body).map(|(d, _)| d);
                                        match data {
                                            Ok(data) => {
                                                //println!("Parsed data: {:?}", data);
                                                let data_size = core::mem::size_of_val(&data);
                                                unsafe {
                                                    let fetched_data_size = FETCHED_DATA_SIZE.get();
                                                    *fetched_data_size += data_size;
                                                }
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
                                    println!("Non-200 HTTP response received");
                                }
                            }
                            Ok(Err(esp_mbedtls::TlsError::Eof)) => {
                                break;
                            }
                            Ok(Err(e)) => {
                                println!("read error: {:?}", e);
                                break;
                            }
                            Err(_) => {
                                println!("Read operation timed out");
                                break;
                            }
                        }

                        // Check if memory is full and pause if necessary
                        if MEMORY_FULL.load(Ordering::SeqCst) {
                            println!("Memory is full, pausing fetch...");
                            while MEMORY_FULL.load(Ordering::SeqCst) {
                                Timer::after(Duration::from_secs(1)).await;
                            }
                            println!("Resuming fetch...");
                        }
                    }

                    // Send all fetched data to the store_data task
                    fetch_sender.send(FetchMessage::FetchedData(all_data)).await;
                }
                Err(e) => {
                    // Detailed error handling for TLS connection failure
                    match e {
                        esp_mbedtls::TlsError::Unknown => {
                            println!("TLS session connection failed: Unknown error");
                        }
                        esp_mbedtls::TlsError::OutOfMemory => {
                            println!("TLS session connection failed: Out of memory");
                        }
                        esp_mbedtls::TlsError::MbedTlsError(code) => {
                            println!("TLS session connection failed: MbedTlsError({})", code);
                            // Optionally, match specific codes if you need more detailed handling
                            match code {
                                -9984 => {
                                    println!("Error detail: CERTIFICATE_VERIFY_FAILED (-9984)");
                                }
                                -0x2700 => {
                                    println!("Error detail: SSL_HANDSHAKE_FAILURE (-0x2700)");
                                }
                                -0x2800 => {
                                    println!("Error detail: SSL_FATAL_ALERT_MESSAGE (-0x2800)");
                                }
                                -0x4200 => {
                                    println!("Error detail: SSL_INTERNAL_ERROR (-0x4200)");
                                }
                                _ => {
                                    println!("Error detail: Unrecognized MbedTlsError code");
                                }
                            }
                        }
                        esp_mbedtls::TlsError::Eof => {
                            println!("TLS session connection failed: EOF encountered");
                        }
                        esp_mbedtls::TlsError::X509MissingNullTerminator => {
                            println!("TLS session connection failed: X509 missing null terminator");
                        }
                        esp_mbedtls::TlsError::NoClientCertificate => {
                            println!(
                                "TLS session connection failed: No client certificate provided"
                            );
                        }
                    }
                }
            }
        }

        Err(e) => {
            println!("Failed to initialize TLS session: {:?}", e);
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