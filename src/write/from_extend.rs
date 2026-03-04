use std::fmt::{self, Debug, Formatter};
use std::io::Error as IoError;
use std::iter::Extend;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::{FusedMultipartWrite, MultipartWrite};

/// Function that constructs a `MultipartWrite` from any value that has a
/// default and implements [`std::iter::Extend`].
pub fn from_extend<A, T: Unpin + Default + Extend<A>>() -> FromExtend<A, T> {
    FromExtend::new(Default::default())
}

/// `MultipartWrite` for the function `from_extend`.
pub struct FromExtend<A, T> {
    inner: Option<T>,
    _a: PhantomData<A>,
}

impl<A, T: Unpin + Default + Extend<A>> FromExtend<A, T> {
    fn new(inner: T) -> Self {
        FromExtend { inner: Some(inner), _a: PhantomData }
    }
}

impl<A, T: Unpin + Default + Extend<A>> Unpin for FromExtend<A, T> {}

impl<A, T> FusedMultipartWrite<A> for FromExtend<A, T>
where
    T: Unpin + Default + Extend<A>,
{
    fn is_terminated(&self) -> bool {
        false
    }
}

impl<A, T> MultipartWrite<A> for FromExtend<A, T>
where
    T: Unpin + Default + Extend<A>,
{
    type Error = IoError;
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

impl<A, T> Debug for FromExtend<A, T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("FromExtend").field("inner", &self.inner).finish()
    }
}
