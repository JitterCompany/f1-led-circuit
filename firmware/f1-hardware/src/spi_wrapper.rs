pub struct SpiWrapper<SPI> {
    spi: SPI,
}

impl<SPI> SpiWrapper<SPI> {
    pub fn new(spi: SPI) -> Self {
        Self { spi }
    }
}

   