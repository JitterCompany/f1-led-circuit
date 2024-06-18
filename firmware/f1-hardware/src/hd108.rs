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
    fn create_led_frame(red: u16, green: u16, blue: u16) -> [u8; 7] {
        [
            0b11100000, // Start code (1 bit) and global brightness (111 for maximum brightness)
            (red >> 8) as u8,
            (red & 0xFF) as u8, // Red (16 bits)
            (green >> 8) as u8,
            (green & 0xFF) as u8, // Green (16 bits)
            (blue >> 8) as u8,
            (blue & 0xFF) as u8, // Blue (16 bits)
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

    pub async fn write_byte(&mut self, word: u8) -> Result<(), SPI::Error> {
        self.spi.write(&[word]).await
    }

    pub async fn write_bytes(&mut self, words: &[u8]) -> Result<(), SPI::Error> {
        self.spi.write(words).await
    }

    pub async fn transfer<'w>(
        &mut self,
        words: &'w mut [u8],
        buffer: &[u8],
    ) -> Result<&'w [u8], SPI::Error> {
        self.spi.transfer(words, buffer).await?;
        Ok(words)
    }
}
