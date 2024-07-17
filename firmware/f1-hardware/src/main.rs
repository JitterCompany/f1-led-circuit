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
use embassy_net::Stack;
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

// Importing necessary TLS modules
use embedded_io_async::Read;
use esp_mbedtls::{asynch::Session, set_debug, Certificates, Mode, TlsVersion, X509};

// Wifi
use embassy_net::{tcp::TcpSocket, Config, StackResources};
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
    #[allow(dead_code)]
    fn from_postcard(bytes: &[u8]) -> Result<Self, &'static str> {
        from_bytes(bytes).map_err(|_| "Failed to deserialize")
    }

    #[allow(dead_code)]
    fn to_postcard(&self) -> Result<Vec<u8, 128>, &'static str> {
        to_vec(self).map_err(|_| "Failed to serialize")
    }
}

enum ButtonMessage {
    ButtonPressed,
}

enum WifiMessage {
    WifiConnected,
    WifiInitialized,
}

enum FetchMessage {
    FetchedData(Vec<FetchedData, 64>), // Dynamically sized vector for the fetched data
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

                    if let Err(e) = spawner.spawn(wifi_connection(
                        controller,
                        stack,
                        wifi_channel.receiver(),
                        wifi_channel.sender(),
                    )) {
                        println!("Failed to spawn wifi_connection: {:?}", e);
                    } else {
                        println!("WiFi Connection spawned...");
                        // Send WifiInitialized message after tasks are spawned
                        wifi_channel
                            .sender()
                            .send(WifiMessage::WifiInitialized)
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
                        wifi_channel.receiver(),
                        stack,
                        fetch_channel.sender(),
                    )) {
                        println!("Failed to spawn fetch_update_frames: {:?}", e);
                    } else {
                        println!("fetch_update_frames spawned...");
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
    receiver: Receiver<'static, NoopRawMutex, WifiMessage, 1>,
    sender: Sender<'static, NoopRawMutex, WifiMessage, 1>,
) {
    println!("Starting wifi connection task");
    println!("Device capabilities: {:?}", controller.get_capabilities());

    // Wait for WifiInitialized message before proceeding
    match receiver.receive().await {
        WifiMessage::WifiInitialized => {
            println!("Wifi initialized message received, proceeding...");
        }
        _ => {}
    }

    loop {
        match esp_wifi::wifi::get_wifi_state() {
            WifiState::StaConnected => {
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after(Duration::from_millis(5000)).await;
            }
            _ => {}
        }

        if !matches!(controller.is_started(), Ok(true)) {
            println!("Configuring wifi client");
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.try_into().unwrap(),
                password: PASSWORD.try_into().unwrap(),
                ..Default::default()
            });

            controller.set_configuration(&client_config).unwrap();
            println!("Client config: {:?}", &client_config);
            println!("Starting wifi");
            controller.start().await.unwrap();
        }

        println!("About to connect...");
        match controller.connect().await {
            Ok(_) => {
                println!("Wifi connected!");

                // Log specific attributes of the controller if possible
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

                // Wait for an IP address
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
                    Timer::after(Duration::from_millis(3000)).await;
                }
            }
            Err(e) => {
                println!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await;
            }
        }
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
    receiver: Receiver<'static, NoopRawMutex, WifiMessage, 1>,
    stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>,
    sender: Sender<'static, NoopRawMutex, FetchMessage, 1>,
) {
    let dns_socket = DnsSocket::new(stack);
    let hostname = "api.openf1.org"; // Replace with your hostname

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
                            let remote_endpoint = (*ip_address, 443); // Using port 443 for HTTPS

                            // Connect to the resolved IP address
                            let mut rx_buffer = [0; 4096];
                            let mut tx_buffer = [0; 4096];
                            let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
                            socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

                            println!("Connecting to {}...", hostname);
                            match socket.connect(remote_endpoint).await {
                                Ok(_) => {
                                    println!("Connected to {}..", hostname);
                                    // Establish a TLS session for HTTPS
                                    fetch_data_https(socket, hostname, sender).await;
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
            _ => {}
        }
    }
}

async fn fetch_data_https(
    mut socket: TcpSocket<'_>,
    hostname: &str,
    sender: Sender<'static, NoopRawMutex, FetchMessage, 1>,
) {
    let session_key = "9149";
    let driver_numbers = [
        1, 2, 4, 10, 11, 14, 16, 18, 20, 22, 23, 24, 27, 31, 40, 44, 55, 63, 77, 81,
    ];
    let start_time = "2023-08-27T12:58:56.234Z";
    let end_time = "2023-08-27T13:20:54.214Z";

    let mut all_data = Vec::<FetchedData, 64>::new();

    // Set debug level for TLS
    set_debug(0);

    // Establish TLS session outside the loop
    let mut tls: Session<_, 4096> = Session::new(
        &mut socket,
        hostname,
        Mode::Client,
        TlsVersion::Tls1_2,
        Certificates {
            ca_chain: X509::pem(concat!(include_str!("api.openf1.org.pem"), "\0").as_bytes())
                .ok(),
            ..Default::default()
        }
    )
    .unwrap();

    println!("Start TLS connect");

    let mut tls = tls.connect().await.unwrap();

    println!("TLS connection established");

    for &driver_number in &driver_numbers {
        let mut url: String<256> = String::new();
        write!(
            url,
            "GET /v1/location?session_key={}&driver_number={}&date%3E{}&date%3C{} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            session_key, driver_number, start_time, end_time, hostname
        )
        .unwrap();

        println!("Sending request: {}", url);
        tls.write_all(url.as_bytes()).await.unwrap();

        let mut response = [0u8; 2048];
        let n = tls.read(&mut response).await.unwrap();
        println!("Raw response length: {}", n);
        println!("Raw response: {:?}", &response[..n]);

        if let Some(body_start) = find_http_body(&response[..n]) {
            let body = &response[body_start..n];
            println!("Body start index: {}", body_start);
            println!("Body length: {}", body.len());
            println!("Body: {:?}", body);

            let data: Result<Vec<FetchedData, 32>, _> = from_slice(body).map(|(d, _)| d);
            match data {
                Ok(data) => {
                    println!("Parsed data length: {}", data.len());
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
    }

    sender.send(FetchMessage::FetchedData(all_data)).await;
}

fn push_u32(buf: &mut String<256>, num: u32) -> Result<(), ()> {
    let mut temp: String<10> = String::new();
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
    let mut data_to_be_visualized: Option<Vec<FetchedData, 64>> = None;

    loop {
        match receiver.receive().await {
            FetchMessage::FetchedData(data) => {
                println!("Received data: {:?}", data);
                data_to_be_visualized = Some(data);
                // Perform any additional processing if necessary
            }
            _ => {}
        }
    }
}
