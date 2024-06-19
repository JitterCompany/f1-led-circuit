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
        let red_gain: u8 = 0b00010; // Regulation level 2 - 2.24 mA
        let green_gain: u8 = 0b00010; // Regulation level 2 - 2.24 mA
        let blue_gain: u8 = 0b00010; // Regulation level 2 - 2.24 mA

        // Combine the gain values into a 15-bit number
        let current_gain =
            ((red_gain as u16) << 10) | ((green_gain as u16) << 5) | (blue_gain as u16);

        // The first byte contains the start code and the 7 most significant bits of the current gain
        let first_byte = (start_code << 7) | ((current_gain >> 8) as u8 & 0x7F);

        // The second byte contains the remaining 8 bits of the current gain
        let second_byte = (current_gain & 0xFF) as u8;

        [
            first_byte,           // Start code and part of current gain
            second_byte,          // Remaining current gain bits
            (red >> 8) as u8,     // High byte of red
            (red & 0xFF) as u8,   // Low byte of red
            (green >> 8) as u8,   // High byte of green
            (green & 0xFF) as u8, // Low byte of green
            (blue >> 8) as u8,    // High byte of blue
            (blue & 0xFF) as u8,  // Low byte of blue
        ]
    }

    pub async fn make_red_green(&mut self) -> Result<(), SPI::Error> {
        // At least 128 bits of zeros for the start frame
        let start_frame = [0x00; 16]; // 16 bytes of zeros = 128 bits

        // Create data frames for all 96 LEDs
        let mut data: Vec<u8, 796> = Vec::new(); // Adjust the size as needed
        data.extend_from_slice(&start_frame).unwrap();

        // Set the first LED to red
        let red_led_frame = Self::create_led_frame(0xFFFF, 0x0000, 0x0000); // Full red intensity
        data.extend_from_slice(&red_led_frame).unwrap();

        // Set the next 95 LEDs to off
        let off_led_frame = Self::create_led_frame(0x0000, 0xFFFF, 0x0000); // Turn off other LEDs
        for _ in 0..95 {
            data.extend_from_slice(&off_led_frame).unwrap();
        }

        // Additional clock pulses equal to the number of LEDs in the strip
        let additional_clocks = [0x00; 12];
        data.extend_from_slice(&additional_clocks).unwrap();

        // Write the data to the SPI bus
        self.spi.write(&data).await?;

        Ok(())
    }

    pub async fn make_red(&mut self, led_num: usize) -> Result<(), SPI::Error> {
        // At least 128 bits of zeros for the start frame
        let start_frame = [0x00; 16]; // 16 bytes of zeros = 128 bits

        // Create data frames for all 96 LEDs
        let mut data: Vec<u8, 796> = Vec::new(); // Adjust the size as needed
        data.extend_from_slice(&start_frame).unwrap();

        // Set the specified LED to red and all others to off
        for i in 0..96 {
            if i == led_num {
                let red_led_frame = Self::create_led_frame(0xFFFF, 0x0000, 0x0000); // Full red intensity
                data.extend_from_slice(&red_led_frame).unwrap();
            } else {
                let off_led_frame = Self::create_led_frame(0x0000, 0x0000, 0x0000); // LED off
                data.extend_from_slice(&off_led_frame).unwrap();
            }
        }

        // Additional clock pulses equal to the number of LEDs in the strip
        let additional_clocks = [0x00; 12];
        data.extend_from_slice(&additional_clocks).unwrap();

        // Write the data to the SPI bus
        self.spi.write(&data).await?;

        Ok(())
    }


    pub async fn set_led(&mut self, led_num: usize, rgb_value: Vec<u8, 3>) -> Result<(), SPI::Error> {
        // At least 128 bits of zeros for the start frame
        let start_frame = [0x00; 16]; 
    
        // Create data frames for all 96 LEDs
        let mut data: Vec<u8, 796> = Vec::new();
        data.extend_from_slice(&start_frame).unwrap();
    
        // Set the specified LED to the given color and all others to off
        for i in 0..96 {
            if i == led_num {
                // Convert the 8-bit RGB values to 16-bit values
                let red = ((rgb_value[0] as u16) << 8) | (rgb_value[0] as u16);
                let green = ((rgb_value[1] as u16) << 8) | (rgb_value[1] as u16);
                let blue = ((rgb_value[2] as u16) << 8) | (rgb_value[2] as u16);
    
                let led_frame = Self::create_led_frame(red, green, blue);
                data.extend_from_slice(&led_frame).unwrap();
            } else {
                let off_led_frame = Self::create_led_frame(0x0000, 0x0000, 0x0000); // LED off
                data.extend_from_slice(&off_led_frame).unwrap();
            }
        }
    
        // Additional clock pulses equal to the number of LEDs in the strip
        let additional_clocks = [0x00; 12];
        data.extend_from_slice(&additional_clocks).unwrap();
    
        // Write the data to the SPI bus
        self.spi.write(&data).await?;
    
        Ok(())
    }
}
