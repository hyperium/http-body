use super::Body;
use bytes::Buf;
use http::HeaderMap;
use std::{
    convert::Infallible,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

/// A body that is always empty.
#[derive(Debug, Clone, Copy)]
pub struct Empty<D> {
    _marker: PhantomData<D>,
}

impl<D> Default for Empty<D> {
    fn default() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<D> Empty<D> {
    /// Create a new `Empty`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl<D: Buf> Body for Empty<D> {
    type Data = D;
    type Error = Infallible;

    #[inline]
    fn poll_data(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        Poll::Ready(None)
    }

    #[inline]
    fn poll_trailers(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        Poll::Ready(Ok(None))
    }
}
