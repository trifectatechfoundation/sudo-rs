//! Binary serialization, and an implementation over Unix pipes.
use sealed::DeSerializeBytes;
use std::{
    io::{self, Read, Write},
    marker::PhantomData,
    os::{fd::AsRawFd, unix::net::UnixStream},
};

mod sealed {
    pub trait DeSerializeBytes {
        fn zero_init() -> Self;
        fn as_mut_ref(&mut self) -> &mut [u8];
    }

    impl<const N: usize> DeSerializeBytes for [u8; N] {
        fn zero_init() -> [u8; N] {
            [0; N]
        }
        fn as_mut_ref(&mut self) -> &mut [u8] {
            self.as_mut_slice()
        }
    }
}

/// Serialization/deserialization trait using a byte array as storage.
pub trait DeSerialize {
    /// Usually `[u8; std::mem::size_of::<Self>()]`.
    type Bytes: sealed::DeSerializeBytes;
    fn serialize(&self) -> Self::Bytes;
    fn deserialize(bytes: Self::Bytes) -> Self;
}

/// A binary pipe that can send and recieve typed messages.
///
/// By default, if only one generic is included,
/// the types of the [BinPipe::write()] and [BinPipe::read()] messages
/// are the same.
pub struct BinPipe<R: DeSerialize, W: DeSerialize = R> {
    sock: UnixStream,
    _read_marker: PhantomData<R>,
    _write_marker: PhantomData<W>,
}

impl<R: DeSerialize, W: DeSerialize> BinPipe<R, W> {
    /// A pipe abstracting over a [UnixStream] with easier
    /// binary serialization, to help with the buffer sizes and ser/de steps.
    /// Uses [UnixStream::pair()].
    pub fn pair() -> io::Result<(BinPipe<R, W>, BinPipe<W, R>)> {
        let (first, second) = UnixStream::pair()?;
        Ok((
            BinPipe {
                sock: first,
                _read_marker: PhantomData::<R>,
                _write_marker: PhantomData::<W>,
            },
            // R and W are inverted here since the type of what's written in one
            // pipe is read in the other, and vice versa.
            BinPipe {
                sock: second,
                _read_marker: PhantomData::<W>,
                _write_marker: PhantomData::<R>,
            },
        ))
    }

    /// Read a `R` from the pipe.
    pub fn read(&mut self) -> io::Result<R> {
        let mut bytes = R::Bytes::zero_init();
        self.sock.read_exact(bytes.as_mut_ref())?;
        Ok(R::deserialize(bytes))
    }

    /// Write a `W` to the pipe.
    pub fn write(&mut self, bytes: &W) -> io::Result<()> {
        self.sock.write_all(bytes.serialize().as_mut_ref())?;
        Ok(())
    }

    /// Calls [std::net::TcpStream::set_nonblocking] on the underlying socket.
    #[cfg(debug_assertions)]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.sock.set_nonblocking(nonblocking)
    }
}

impl<R: DeSerialize, W: DeSerialize> AsRawFd for BinPipe<R, W> {
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.sock.as_raw_fd()
    }
}

impl DeSerialize for i32 {
    type Bytes = [u8; std::mem::size_of::<Self>()];

    fn serialize(&self) -> Self::Bytes {
        self.to_ne_bytes()
    }
    fn deserialize(bytes: Self::Bytes) -> Self {
        Self::from_ne_bytes(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn single_type() {
        let (mut tx, mut rx) = BinPipe::pair().unwrap();
        tx.write(&42i32).unwrap();
        assert_eq!(rx.read().unwrap(), 42);
        rx.write(&23i32).unwrap();
        assert_eq!(tx.read().unwrap(), 23);
    }

    #[test]
    pub fn different_types() {
        impl DeSerialize for u8 {
            type Bytes = [u8; std::mem::size_of::<Self>()];
            fn serialize(&self) -> [u8; 1] {
                self.to_ne_bytes()
            }
            fn deserialize(bytes: [u8; 1]) -> Self {
                Self::from_ne_bytes(bytes)
            }
        }

        let (mut tx, mut rx) = BinPipe::pair().unwrap();
        tx.write(&42i32).unwrap();
        assert_eq!(rx.read().unwrap(), 42);
        rx.write(&23u8).unwrap();
        assert_eq!(tx.read().unwrap(), 23);
    }
}
