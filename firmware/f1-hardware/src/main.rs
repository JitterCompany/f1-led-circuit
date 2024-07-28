#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod f1_data;
mod hd108;
//use data::VISUALIZATION_DATA;
use f1_data::{DriverInfo, Entry};
use core::{fmt::Write as _, ptr::addr_of_mut, str};
use embassy_net::{tcp::TcpSocket, Config, Ipv4Address, Stack, StackResources};
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer as EmbassyTimer};
use embedded_io_async::Write as _;
use embedded_tls::{Aes128GcmSha256, NoVerify, TlsConfig, TlsConnection, TlsContext};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::channel::Receiver;
use embassy_sync::channel::Sender;
use embedded_hal_async::spi::SpiBus;
//use esp_backtrace as _;
use esp_hal::dma::DmaDescriptor;
use esp_hal::spi::master::prelude::_esp_hal_spi_master_dma_WithDmaSpi2;
use esp_hal::{
    clock::ClockControl,
    entry,
    dma::{Dma, DmaPriority},
    gpio::{Event, GpioPin, Input, Io, Pull},
    peripherals::Peripherals,
    prelude::{_esp_hal_timer_Timer, main},
    spi::{master::Spi, SpiMode},
    rng::Rng,
    system::SystemControl,
    timer::timg::TimerGroup,
};
use esp_wifi::{
    initialize,
    wifi::{ClientConfiguration, Configuration, WifiDevice, WifiStaDevice},
    EspWifiInitFor,
};
use esp_println::println;
use hd108::HD108;
use heapless::Vec;
use panic_halt as _;
use rand_core::{CryptoRng, Error as RandError, RngCore};
use static_cell::StaticCell;
use heapless::String;
use fugit::HertzU32;

// LED
struct RGBColor {
    r: u8,
    g: u8,
    b: u8,
}

// Messages in Async Channels
enum Message {
    ButtonPressed,
}

// Init Channels for Async Messages

static SIGNAL_CHANNEL: StaticCell<Channel<NoopRawMutex, Message, 1>> = StaticCell::new();

// WiFi
const SSID: &str = "SSID";
const PASSWORD: &str = "PASSWORD";

const CONNECT_ATTEMPTS: usize = 10;
const RETRY_DELAY_MS: u64 = 5000;

type Entries = heapless::Vec<Entry, 16>;

// Custom RNG implementation for debugging
pub struct SimpleRng {
    counter: u32, // Simple counter for pseudo-random numbers
}

impl SimpleRng {
    pub fn new() -> Self {
        Self { counter: 0 }
    }
}

impl RngCore for SimpleRng {
    fn next_u32(&mut self) -> u32 {
        // Simple counter-based pseudo-random number generator
        self.counter = self.counter.wrapping_add(1);
        self.counter
    }

    fn next_u64(&mut self) -> u64 {
        let upper = self.next_u32() as u64;
        let lower = self.next_u32() as u64;
        (upper << 32) | lower
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(4) {
            let rand = self.next_u32();
            let bytes = rand.to_ne_bytes();
            for (i, byte) in chunk.iter_mut().enumerate() {
                *byte = bytes[i];
            }
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), RandError> {
        self.fill_bytes(dest);
        Ok(())
    }
}

impl CryptoRng for SimpleRng {}


#[main]
async fn main(spawner: Spawner) {
    println!("Starting program!...");

    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);
    //let clocks = ClockControl::boot_defaults(system.clock_control).freeze();
    let clocks = ClockControl::max(system.clock_control).freeze();

    let timg0 = TimerGroup::new_async(peripherals.TIMG0, &clocks);
    esp_hal_embassy::init(&clocks, timg0);

    let timer = esp_hal::timer::systimer::SystemTimer::new(peripherals.SYSTIMER).alarm0;

    // Start the timer
    //timer0.start();

    // Initialize RNG peripherial
    let rng = Rng::new(peripherals.RNG);

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

    let spi = Spi::new(peripherals.SPI2, HertzU32::from_raw(20_000_000), SpiMode::Mode0, &clocks)
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

    // Spawn the led task with the receiver
    spawner
        .spawn(led_task(hd108, signal_channel.receiver()))
        .unwrap();

        let init = match initialize(
            EspWifiInitFor::Wifi,
            timer,
            rng,
            peripherals.RADIO_CLK,
            &clocks,
        ) {
            Ok(init) => {
                println!("Wi-Fi initialization successful.");
                init
            }
            Err(e) => {
                println!("Wi-Fi initialization failed: {:?}", e);
                return;
            }
        };
    
        let wifi = peripherals.WIFI;
        let (wifi_interface, mut controller) =
            esp_wifi::wifi::new_with_mode(&init, wifi, WifiStaDevice).unwrap();
    
        let mut ssid: String<32> = String::new();
        let mut password: String<64> = String::new();
        ssid.push_str(SSID).unwrap();
        password.push_str(PASSWORD).unwrap();
    
        let client_config = ClientConfiguration {
            ssid,
            password,
            ..Default::default()
        };
    
        controller
            .set_configuration(&Configuration::Client(client_config))
            .unwrap();
        controller.start().await.unwrap();
        println!("WiFi Started...");
    
        let mut attempts = 0;
        loop {
            attempts += 1;
            println!("Attempt {}: Connecting to Wi-Fi...", attempts);
    
            if let Ok(()) = controller.connect().await {
                // After starting Wi-Fi and setting configuration
                if let Ok(is_connected) = controller.is_connected() {
                    if is_connected {
                        println!("Wi-Fi connected successfully.");
                    } else {
                        println!("Wi-Fi is not connected.");
                    }
                } else {
                    println!("Error checking Wi-Fi connection status.");
                }
                break;
            }
    
            if attempts >= CONNECT_ATTEMPTS {
                println!(
                    "Failed to connect to Wi-Fi after {} attempts.",
                    CONNECT_ATTEMPTS
                );
                return;
            }
    
            println!("Retrying in {} ms...", RETRY_DELAY_MS);
            EmbassyTimer::after(Duration::from_millis(RETRY_DELAY_MS)).await;
        }
    
        let config = Config::dhcpv4(Default::default());
        let seed = 1234;
    
        static STACK: StaticCell<Stack<WifiDevice<'_, WifiStaDevice>>> = StaticCell::new();
        static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
        let stack = &*STACK.init(Stack::new(
            wifi_interface,
            config,
            RESOURCES.init(StackResources::<3>::new()),
            seed,
        ));
    
        // Launch network task that runs `stack.run().await`
        spawner.spawn(net_task(stack)).unwrap();
        // Wait for DHCP config
        stack.wait_config_up().await;
    
        // Check the stack configuration
        let config_v4 = stack.config_v4();
    
        if let Some(config) = config_v4 {
            println!("IP Address: {:?}", config.address);
        } else {
            println!("Failed to obtain IP address.");
        }
    
        println!("Stack IP Configuration: {:?}", stack.config_v4());
    
        // TLS connection setup
        static mut RX_BUFFER_TLS: [u8; 16640] = [0; 16640];
        static mut TX_BUFFER_TLS: [u8; 8192] = [0; 8192];
    
        // Create a new TCP socket
        static mut RX_BUFFER_SOCKET: [u8; 1024] = [0; 1024];
        static mut TX_BUFFER_SOCKET: [u8; 1024] = [0; 1024];
    
        let (rx_buffer_tls, tx_buffer_tls, rx_buffer_socket, tx_buffer_socket) = unsafe {
            (
                &mut *addr_of_mut!(RX_BUFFER_TLS),
                &mut *addr_of_mut!(TX_BUFFER_TLS),
                &mut *addr_of_mut!(RX_BUFFER_SOCKET),
                &mut *addr_of_mut!(TX_BUFFER_SOCKET),
            )
        };
    
        let mut socket = TcpSocket::new(stack, rx_buffer_socket, tx_buffer_socket);
    
        if let Err(e) = socket
            .connect((Ipv4Address::new(35, 241, 27, 1), 443))
            .await
        {
            println!("Failed to connect to the server: {:?}", e);
            return;
        }
    
        let config: TlsConfig<'_, Aes128GcmSha256> = TlsConfig::new().enable_rsa_signatures();
        let mut tls = TlsConnection::new(socket, rx_buffer_tls, tx_buffer_tls);
        println!("Starting TLS handshake...");
        if let Err(e) = tls
            .open::<SimpleRng, NoVerify>(TlsContext::new(&config, &mut SimpleRng::new()))
            .await
        {
            println!("TLS handshake failed: {:?}", e);
            return;
        }
    
        println!("TLS handshake completed successfully. Sending HTTP requests…");
        for driver in f1_data::DRIVERS {
            let mut query = heapless::String::<128>::new();
    
            write!(&mut query, "/v1/location?session_key=9161&driver_number={}&date>2023-09-16T13:03:35.200&date<2023-09-16T13:03:35.800", driver.number).expect("could not format string (probably: buffer too short)");
            for part in [
                b"GET ",
                query.as_bytes(),
                b" HTTP/1.1\r\nHost: api.openf1.org\r\n\r\n",
            ] {
                if let Err(e) = tls.write_all(part).await {
                    println!("Failed to send HTTP request: {:?}", e);
                    return;
                }
            }
    
            println!("sent.");
            tls.flush().await.expect("error flushing data");
            println!("flushed.");
            let mut response = [0; 1024];
            match tls.read(&mut response).await {
                Ok(size) => {
                    if size == 0 {
                        println!("Received no data from the server.");
                    } else {
                        let response_s =
                            str::from_utf8(&response[..size]).unwrap_or("Invalid UTF-8 response");
                        // println!("Response: {}", response_s);
    
                        if let Some(end_of_headers) = response_s.find("\r\n\r\n") {
                            match serde_json_core::de::from_str::<Entries>(
                                &response_s[(end_of_headers + 4)..],
                            ) {
                                Ok((data, _consumed)) => {
                                    println!("got {data:?}");
                                }
                                Err(e) => log::error!("fail: {e:?}"),
                            }
                        } else {
                            log::error!("end of headers not found")
                        }
                    }
                }
                Err(e) => {
                    println!("Failed to read from TLS connection: {:?}", e);
                }
            }
        }
        println!("done!");
    }
    
#[embassy_executor::task]
async fn net_task(stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>) {
    stack.run().await
}

#[embassy_executor::task]
async fn led_task(
    mut hd108: HD108<impl SpiBus<u8> + 'static>,
    receiver: Receiver<'static, NoopRawMutex, Message, 1>,
) {

    let driver_colors = get_driver_colors();

    loop {
        // Wait for the start message
        receiver.receive().await;
        for i in 0..=96 {
            let color = &driver_colors[i % driver_colors.len()]; // Get the corresponding color
            hd108.set_led(i, color.r, color.g, color.b).await.unwrap(); // Pass the RGB values directly

            // Check for a stop message
            if receiver.try_receive().is_ok() {
                hd108.set_off().await.unwrap();
                break;
            }
            EmbassyTimer::after(Duration::from_millis(25)).await; // Debounce delay
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
        EmbassyTimer::after(Duration::from_millis(400)).await; // Debounce delay
    }
}

// Extract driver colors
fn get_driver_colors() -> Vec<RGBColor, 20> {
    let mut colors = Vec::new();
    for driver in f1_data::DRIVERS {
        colors.push(RGBColor {
            r: driver.color.0,
            g: driver.color.1,
            b: driver.color.2,
        });
    }
    colors
}
