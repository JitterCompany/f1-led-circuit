use embedded_hal_async::spi::SpiBus;
use esp_hal::dma::{Channel, DmaTransferRx, DmaTransferTx, ChannelTypes};
use esp_hal::peripherals::SPI2;
use esp_hal::spi::FullDuplexMode;
use esp_hal::spi::master::Spi;
use esp_hal::Mode;
use embedded_hal::spi::ErrorKind;


#[derive(Debug)]
pub struct CustomSpiError;

impl embedded_hal::spi::Error for CustomSpiError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

pub struct AsyncSpiDma<'a, Tx: ChannelTypes + Mode, Rx: ChannelTypes + Mode> {
    spi: Spi<'a, SPI2, FullDuplexMode>,
    tx_channel: Channel<'a, Tx, DmaTransferTx<'a, Tx>>,
    rx_channel: Channel<'a, Rx, DmaTransferRx<'a, Rx>>,
}

impl<'a, Tx: ChannelTypes + Mode, Rx: ChannelTypes + Mode> AsyncSpiDma<'a, Tx, Rx> {
    pub fn new(spi: Spi<'a, SPI2, FullDuplexMode>, tx_channel: Channel<'a, Tx, DmaTransferTx<'a, Tx>>, rx_channel: Channel<'a, Rx, DmaTransferRx<'a, Rx>>) -> Self {
        Self { spi, tx_channel, rx_channel }
    }

    async fn read_internal(&mut self, words: &mut [u8]) -> Result<(), CustomSpiError> {
        self.spi.read_dma(words, &mut self.rx_channel).await.map_err(|_| CustomSpiError)
    }

    async fn write_internal(&mut self, words: &[u8]) -> Result<(), CustomSpiError> {
        self.spi.write_dma(words, &mut self.tx_channel).await.map_err(|_| CustomSpiError)
    }

    async fn transfer_internal(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), CustomSpiError> {
        self.spi.transfer_dma(write, read, &mut self.tx_channel, &mut self.rx_channel).await.map_err(|_| CustomSpiError)
    }

    async fn transfer_in_place_internal(&mut self, words: &mut [u8]) -> Result<(), CustomSpiError> {
        self.spi.transfer_in_place_dma(words, &mut self.tx_channel, &mut self.rx_channel).await.map_err(|_| CustomSpiError)
    }

    async fn flush_internal(&mut self) -> Result<(), CustomSpiError> {
        self.spi.flush().await.map_err(|_| CustomSpiError)
    }
}

impl<'a, Tx: ChannelTypes + Mode, Rx: ChannelTypes + Mode> embedded_hal_async::spi::ErrorType for AsyncSpiDma<'a, Tx, Rx> {
    type Error = CustomSpiError;
}

impl<'a, Tx: ChannelTypes + Mode, Rx: ChannelTypes + Mode> SpiBus<u8> for AsyncSpiDma<'a, Tx, Rx> {
    async fn read(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        self.read_internal(words).await
    }

    async fn write(&mut self, words: &[u8]) -> Result<(), Self::Error> {
        self.write_internal(words).await
    }

    async fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Self::Error> {
        self.transfer_internal(read, write).await
    }

    async fn transfer_in_place(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        self.transfer_in_place_internal(words).await
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.flush_internal().await
    }
}
