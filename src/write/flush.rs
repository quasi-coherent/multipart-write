use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::future::{FusedFuture, Future};
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Future for [`flush`](super::MultipartWriteExt::flush).
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub struct Flush<'a, Wr: ?Sized, Part> {
    writer: &'a mut Wr,
    _p: PhantomData<fn(Part)>,
}

impl<Wr: ?Sized + Unpin, Part> Unpin for Flush<'_, Wr, Part> {}

impl<'a, Wr: ?Sized + MultipartWrite<Part> + Unpin, Part> Flush<'a, Wr, Part> {
    pub(super) fn new(writer: &'a mut Wr) -> Self {
        Self {
            writer,
            _p: std::marker::PhantomData,
        }
    }
}

impl<Wr, Part> FusedFuture for Flush<'_, Wr, Part>
where
    Wr: FusedMultipartWrite<Part> + Unpin,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated()
    }
}

impl<Wr: MultipartWrite<Part> + Unpin, Part> Future for Flush<'_, Wr, Part> {
    type Output = Result<(), Wr::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.writer).poll_flush(cx)
    }
}
