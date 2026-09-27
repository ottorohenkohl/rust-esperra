use embedded_io_async::{ErrorType, Read, Write};

/// Preserve the IO behavior of independently owned read/write halves.
pub(super) struct Duplex<R, W> {
    reader: R,
    writer: W,
}

impl<R, W> Duplex<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        Self { reader, writer }
    }

    pub fn split(&mut self) -> (&mut R, &mut W) {
        (&mut self.reader, &mut self.writer)
    }
}

impl<R: ErrorType, W: ErrorType<Error = R::Error>> ErrorType for Duplex<R, W> {
    type Error = R::Error;
}

impl<R: Read, W: ErrorType<Error = R::Error>> Read for Duplex<R, W> {
    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        self.reader.read(buffer).await
    }
}

impl<R: ErrorType, W: Write<Error = R::Error>> Write for Duplex<R, W> {
    async fn write(&mut self, buffer: &[u8]) -> Result<usize, Self::Error> {
        self.writer.write(buffer).await
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.writer.flush().await
    }
}
