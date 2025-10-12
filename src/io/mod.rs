//! `MultipartWrite` for foreign writer types.
use std::io::Write;

#[cfg(feature = "tokio-io")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio-io")))]
mod multi_async_writer;
#[cfg(feature = "tokio-io")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio-io")))]
#[doc(inline)]
pub use multi_async_writer::{MultiAsyncWriter, async_write};

mod multi_io_writer;
#[doc(inline)]
pub use multi_io_writer::MultiIoWriter;

/// Converts a [`Write`] into a [`MultipartWrite`] that is always available to
/// have parts written.
///
/// [`Write`]: std::io::Write
/// [`MultipartWrite`]: crate::MultipartWrite
pub fn io_write<W: Write + Default>(write: W) -> MultiIoWriter<W> {
    MultiIoWriter::new(write)
}
