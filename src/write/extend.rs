use crate::MultipartWrite;

use std::fmt::{self, Debug, Formatter};
use std::io::Error as IoError;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Converts an `Extend` into a `MultipartWrite` that is always ready to receive
/// the next part to write.
pub fn extend<T, A>(inner: T) -> Extend<T, A>
where
    T: std::iter::Extend<A>,
{
    Extend::new(inner)
}

pin_project_lite::pin_project! {
    /// `MultipartWrite` for the [`extend`] function.
    #[must_use = "futures do nothing unless polled"]
    pub struct Extend<T, A> {
        inner: Option<T>,
        _a: PhantomData<A>,
    }
}

impl<T, A> Extend<T, A> {
    fn new(inner: T) -> Self {
        Extend {
            inner: Some(inner),
            _a: PhantomData,
        }
    }
}

impl<T, A> MultipartWrite<A> for Extend<T, A>
where
    T: std::iter::Extend<A>,
{
    type Error = IoError;
    type Ret = ();
    type Output = T;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(mut self: Pin<&mut Self>, part: A) -> Result<Self::Ret, Self::Error> {
        let Some(ext) = self.inner.as_mut() else {
            return Err(IoError::other("unset"));
        };
        ext.extend([part]);
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_complete(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        let out = self.inner.take();
        if out.is_none() {
            return Poll::Ready(Err(IoError::other("unset")));
        }
        Poll::Ready(Ok(out.unwrap()))
    }
}

impl<T, A> Debug for Extend<T, A>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Extend")
            .field("inner", &self.inner)
            .finish()
    }
}
