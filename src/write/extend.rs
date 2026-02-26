use std::convert::Infallible as Never;
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::{FusedMultipartWrite, MultipartWrite};

/// Returns a value that becomes a `MultipartWrite` over any `A` where `T:
/// std::iter::Extend<A>`.
pub fn extend<T: Unpin + Default>(init: T) -> Extend<T> {
    Extend::new(init)
}

/// [`extend`] but starts with the default value of `T`.
pub fn extend_default<T: Unpin + Default>() -> Extend<T> {
    Extend::new(T::default())
}

/// `MultipartWrite` for [`extend`].
pub struct Extend<T> {
    inner: Option<T>,
}

impl<T: Unpin + Default> Extend<T> {
    fn new(inner: T) -> Self {
        Self { inner: Some(inner) }
    }
}

impl<T: Unpin + Default> Unpin for Extend<T> {}

impl<A, T> FusedMultipartWrite<A> for Extend<T>
where
    T: Unpin + Default + std::iter::Extend<A>,
{
    fn is_terminated(&self) -> bool {
        false
    }
}

impl<A, T> MultipartWrite<A> for Extend<T>
where
    T: Unpin + Default + std::iter::Extend<A>,
{
    type Error = Never;
    type Output = T;
    type Recv = ();

    fn poll_ready(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        part: A,
    ) -> Result<Self::Recv, Self::Error> {
        match self.inner.as_mut() {
            Some(vs) => vs.extend([part]),
            _ => {
                let mut vs = T::default();
                vs.extend([part]);
                self.inner = Some(vs);
            },
        }
        Ok(())
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_complete(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        Poll::Ready(Ok(self.inner.take().unwrap_or_default()))
    }
}

impl<T> Debug for Extend<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Extend").field("inner", &self.inner).finish()
    }
}
