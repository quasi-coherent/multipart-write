use crate::{FusedMultipartWrite, MultipartWrite};

use futures::{Future, ready};
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

/// `MultipartWrite` for [`on_complete`].
///
/// [`on_complete`]: super::MultipartWriteExt::on_complete
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project]
pub struct OnComplete<Wr, S, F, Fut> {
    #[pin]
    writer: Option<Wr>,
    f: F,
    s: S,
    #[pin]
    future: Option<Fut>,
    is_terminated: bool,
    _f: std::marker::PhantomData<fn(S)>,
}

impl<Wr, S, F, Fut> OnComplete<Wr, S, F, Fut> {
    pub(super) fn new(writer: Wr, s: S, f: F) -> Self {
        Self {
            writer: Some(writer),
            f,
            s,
            future: None,
            is_terminated: false,
            _f: std::marker::PhantomData,
        }
    }

    fn try_poll_ready<Part>(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Wr::Error>>
    where
        Wr: MultipartWrite<Part>,
        F: FnMut(&mut S) -> Fut,
        Fut: Future<Output = Result<Wr, Wr::Error>>,
    {
        let mut this = self.project();

        if this.writer.is_some() {
            let wr = this.writer.as_mut().as_pin_mut().unwrap();
            return wr.poll_ready(cx);
        }

        if this.future.is_some() {
            let fut = this.future.as_mut().as_pin_mut().unwrap();
            match ready!(fut.poll(cx)) {
                Ok(wr) => {
                    this.writer.set(Some(wr));
                    this.future.set(None);
                    // Return `Poll::Pending`, not `Poll::Ready(Ok(()))` because
                    // the writer is set now, but that doesn't mean it's ready.
                    // On the next poll it will hit the top and call `poll_ready`
                    // on the writer.
                    Poll::Pending
                }
                Err(e) => {
                    this.writer.set(None);
                    this.future.set(None);
                    *this.is_terminated = true;
                    Poll::Ready(Err(e))
                }
            }
        } else {
            let fut = (this.f)(this.s);
            this.future.set(Some(fut));
            Poll::Pending
        }
    }
}

impl<Wr, S, F, Fut, Part> FusedMultipartWrite<Part> for OnComplete<Wr, S, F, Fut>
where
    Wr: FusedMultipartWrite<Part>,
    F: FnMut(&mut S) -> Fut,
    Fut: Future<Output = Result<Wr, Wr::Error>>,
{
    fn is_terminated(&self) -> bool {
        self.is_terminated
    }
}

impl<Wr, S, F, Fut, Part> MultipartWrite<Part> for OnComplete<Wr, S, F, Fut>
where
    Wr: MultipartWrite<Part>,
    F: FnMut(&mut S) -> Fut,
    Fut: Future<Output = Result<Wr, Wr::Error>>,
{
    type Ret = Wr::Ret;
    type Output = Wr::Output;
    type Error = Wr::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.try_poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error> {
        let mut this = self.project();
        let wr = this
            .writer
            .as_mut()
            .as_pin_mut()
            .expect("called start_send without poll_ready");
        wr.start_send(part)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let mut this = self.project();
        let wr = this
            .writer
            .as_mut()
            .as_pin_mut()
            .expect("polled OnComplete after completion");
        wr.poll_flush(cx)
    }

    fn poll_complete(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        let mut this = self.project();
        let wr = this
            .writer
            .as_mut()
            .as_pin_mut()
            .expect("polled OnComplete after completion");
        let output = ready!(wr.poll_complete(cx));
        this.writer.set(None);

        if output.is_err() {
            this.future.set(None);
            *this.is_terminated = true;
        } else {
            let fut = (this.f)(this.s);
            this.future.set(Some(fut));
        }

        Poll::Ready(output)
    }
}

impl<Wr, S, F, Fut> Debug for OnComplete<Wr, S, F, Fut>
where
    Wr: Debug,
    S: Debug,
    Fut: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("OnComplete")
            .field("writer", &self.writer)
            .field("s", &self.s)
            .field("future", &self.future)
            .field("is_terminated", &self.is_terminated)
            .finish()
    }
}
