use crate::MultipartWrite;

use futures::ready;
use std::pin::Pin;
use std::task::{Context, Poll};

/// `MultipartWrite` for [`fanout`](super::MultipartWriteExt::fanout).
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project]
pub struct Fanout<W: MultipartWrite<Part>, U: MultipartWrite<Part>, Part> {
    #[pin]
    w: W,
    #[pin]
    u: U,
    wo: Option<W::Output>,
    uo: Option<U::Output>,
}

impl<W: MultipartWrite<Part>, U: MultipartWrite<Part>, Part> Fanout<W, U, Part> {
    pub(super) fn new(w: W, u: U) -> Self {
        Self {
            w,
            u,
            wo: None,
            uo: None,
        }
    }
}

impl<W, U, Part> MultipartWrite<Part> for Fanout<W, U, Part>
where
    Part: Clone,
    W: MultipartWrite<Part>,
    U: MultipartWrite<Part, Error = W::Error>,
{
    type Ret = (W::Ret, U::Ret);
    type Output = (W::Output, U::Output);
    type Error = W::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        let w_ready = this.w.poll_ready(cx)?.is_ready();
        let u_ready = this.u.poll_ready(cx)?.is_ready();
        if w_ready && u_ready {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }

    fn start_send(self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error> {
        let this = self.project();
        let w_ret = this.w.start_send(part.clone())?;
        let u_ret = this.u.start_send(part)?;
        Ok((w_ret, u_ret))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        let w_ready = this.w.poll_flush(cx)?.is_ready();
        let u_ready = this.u.poll_flush(cx)?.is_ready();
        if w_ready && u_ready {
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
        let w_output = ready!(this.w.poll_complete(cx))?;
        *this.wo = Some(w_output);
        let u_output = ready!(this.u.poll_complete(cx))?;
        Poll::Ready(Ok((this.wo.take().unwrap(), u_output)))
    }
}
