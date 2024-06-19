use embedded_hal_async::spi::SpiBus;
use heapless::Vec;

pub struct HD108<SPI> {
    pub spi: SPI,
}

impl<SPI> HD108<SPI>
where
    SPI: SpiBus<u8>,
{
    pub fn new(spi: SPI) -> Self {
        Self { spi }
    }

    // Function to create an LED frame
fn create_led_frame(red: u16, green: u16, blue: u16) -> [u8; 8] {
    let start_code: u8 = 0b1;
    // Regulation level 4 - 3.36 mA
    let red_gain: u8 = 0b00011; 
    let green_gain: u8 = 0b00011; 
    let blue_gain: u8 = 0b00011; 

    // Combine the gain values into a 15-bit number
    let current_gain = ((red_gain as u16) << 10) | ((green_gain as u16) << 5) | (blue_gain as u16);

    // The first byte contains the start code and the 7 most significant bits of the current gain
    let first_byte = (start_code << 7) | ((current_gain >> 8) as u8 & 0x7F);

    [
        first_byte,                  // Start code and part of current gain
        (current_gain & 0xFF) as u8, // Remaining current gain bits
        (red >> 8) as u8,            // High byte of red
        (red & 0xFF) as u8,          // Low byte of red
        (green >> 8) as u8,          // High byte of green
        (green & 0xFF) as u8,        // Low byte of green
        (blue >> 8) as u8,           // High byte of blue
        (blue & 0xFF) as u8,         // Low byte of blue
    ]
}

    pub async fn make_red(&mut self) -> Result<(), SPI::Error> {
        // At least 128 bits of zeros for the start frame
        let start_frame = [0x00; 16]; // 16 bytes of zeros = 128 bits

        // Create data frames for all 96 LEDs
        let mut data: Vec<u8, 768> = Vec::new(); // Adjust the size as needed
        data.extend_from_slice(&start_frame).unwrap();

        // Set the first LED to red 
        let red_led_frame = Self::create_led_frame(0xFFFF, 0x0000, 0x0000);
        data.extend_from_slice(&red_led_frame).unwrap();

        // Set the remaining 95 LEDs to off
        let off_led_frame = Self::create_led_frame(0x0000, 0x0000, 0x0000);
        for _ in 0..95 {
            data.extend_from_slice(&off_led_frame).unwrap();
        }

        let end_frame = [0x00; 16]; // 128 bits of zeros
        data.extend_from_slice(&end_frame).unwrap();

        // Write the data to the SPI bus
        self.spi.write(&data).await?;

        Ok(())
    }


}
