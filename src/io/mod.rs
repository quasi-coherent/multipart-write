//! Foreign writer types.
//!
//! This module implements `MultipartWrite` for the `tokio` and `std` writer
//! types, and it exports constructors for creating these implementations.
use std::io::Write;

#[cfg(feature = "tokio")]
mod multi_async_writer;
#[cfg(feature = "tokio")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio")))]
#[doc(inline)]
pub use multi_async_writer::{MultiAsyncWriter, async_writer};

mod multi_io_writer;
pub use multi_io_writer::MultiIoWriter;

/// Constructs a `MultipartWrite` from an `std::io::Write`.
pub fn io_writer<W: Write + Default>(write: W) -> MultiIoWriter<W> {
    MultiIoWriter::new(write)
}
