pub mod datetime;
pub mod dir;
pub mod error;
pub mod fat;
pub mod file;
pub mod fs;
pub mod partition;
pub mod path;

pub use datetime::DateTime;
pub use dir::DirectoryEntry;
pub use error::Error;
pub use file::File;
pub use fs::{FatxFs, FatxFsConfig, FatxFsHandle};
pub use partition::{DEFAULT_PARTITION_LAYOUT, PartitionMapEntry};

use std::io::{self, Read};
use zerocopy::FromBytes;

pub(crate) fn read_struct<R: Read, T: FromBytes>(mut reader: R) -> io::Result<T> {
    let mut buf = vec![0u8; std::mem::size_of::<T>()];
    reader.read_exact(&mut buf)?;
    T::read_from_bytes(&buf)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Failed to parse struct from bytes"))
}

