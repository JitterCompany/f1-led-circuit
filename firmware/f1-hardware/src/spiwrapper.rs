use embedded_hal_async::spi::{SpiBus as AsyncSpiBus, ErrorType};
use embedded_hal::spi::Error;
use core::marker::PhantomData;

pub struct AsyncSpiBusWrapper<T, E> {
    spi: T,
    _error: PhantomData<E>,
}

impl<T, E> AsyncSpiBusWrapper<T, E>
where
    E: Error + 'static,
    T: AsyncSpiBus<u8, Error = E>,
{
    pub fn new(spi: T) -> Self {
        Self {
            spi,
            _error: PhantomData,
        }
    }
}

impl<T, E> ErrorType for AsyncSpiBusWrapper<T, E>
where
    E: Error + 'static,
{
    type Error = E;
}

impl<T, E> AsyncSpiBus<u8> for AsyncSpiBusWrapper<T, E>
where
    E: Error + 'static,
    T: AsyncSpiBus<u8, Error = E>,
{
    async fn read(&mut self, words: &mut [u8]) -> Result<(), E> {
        self.spi.read(words).await
    }

    async fn write(&mut self, words: &[u8]) -> Result<(), E> {
        self.spi.write(words).await
    }

    async fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), E> {
        self.spi.transfer(read, write).await
    }

    async fn transfer_in_place(&mut self, words: &mut [u8]) -> Result<(), E> {
        self.spi.transfer_in_place(words).await
    }

    async fn flush(&mut self) -> Result<(), E> {
        self.spi.flush().await
    }
}
