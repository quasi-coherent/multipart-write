use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::ready;
use std::pin::Pin;
use std::task::{Context, Poll};

/// `MultipartWrite` for [`fanout`](super::MultipartWriteExt::fanout).
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project]
pub struct Fanout<Wr1: MultipartWrite<Part>, Wr2: MultipartWrite<Part>, Part> {
    #[pin]
    wr1: Wr1,
    #[pin]
    wr2: Wr2,
    wro1: Option<Wr1::Output>,
    wro2: Option<Wr2::Output>,
}

impl<Wr1: MultipartWrite<Part>, Wr2: MultipartWrite<Part>, Part> Fanout<Wr1, Wr2, Part> {
    pub(super) fn new(wr1: Wr1, wr2: Wr2) -> Self {
        Self {
            wr1,
            wr2,
            wro1: None,
            wro2: None,
        }
    }
}

impl<Wr1, Wr2, Part> FusedMultipartWrite<Part> for Fanout<Wr1, Wr2, Part>
where
    Part: Clone,
    Wr1: FusedMultipartWrite<Part>,
    Wr2: FusedMultipartWrite<Part, Error = Wr1::Error>,
{
    fn is_terminated(&self) -> bool {
        self.wr1.is_terminated() || self.wr2.is_terminated()
    }
}

impl<Wr1, Wr2, Part> MultipartWrite<Part> for Fanout<Wr1, Wr2, Part>
where
    Part: Clone,
    Wr1: MultipartWrite<Part>,
    Wr2: MultipartWrite<Part, Error = Wr1::Error>,
{
    type Ret = (Wr1::Ret, Wr2::Ret);
    type Output = (Wr1::Output, Wr2::Output);
    type Error = Wr1::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        let ready1 = this.wr1.poll_ready(cx)?.is_ready();
        let ready2 = this.wr2.poll_ready(cx)?.is_ready();
        if ready1 && ready2 {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }

    fn start_send(self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error> {
        let this = self.project();
        let ret1 = this.wr1.start_send(part.clone())?;
        let ret2 = this.wr2.start_send(part)?;
        Ok((ret1, ret2))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        let ready1 = this.wr1.poll_flush(cx)?.is_ready();
        let ready2 = this.wr2.poll_flush(cx)?.is_ready();
        if ready1 && ready2 {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }

    fn poll_complete(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        let this = self.project();
        let output1 = ready!(this.wr1.poll_complete(cx))?;
        *this.wro1 = Some(output1);
        let output2 = ready!(this.wr2.poll_complete(cx))?;
        Poll::Ready(Ok((this.wro1.take().unwrap(), output2)))
    }
}
