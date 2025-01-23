use bytes::Buf;
use http_body::{Body, Frame, SizeHint};
use std::{
    convert::Infallible,
    fmt,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

/// A body that is always empty.
pub struct Empty<D, E = Infallible> {
    _marker: PhantomData<fn() -> (D, E)>,
}

impl<D, E> Empty<D, E> {
    /// Create a new `Empty`.
    pub const fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<D: Buf, E> Body for Empty<D, E> {
    type Data = D;
    type Error = E;

    #[inline]
    fn poll_frame(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Poll::Ready(None)
    }

    fn is_end_stream(&self) -> bool {
        true
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(0)
    }
}

impl<D, E> fmt::Debug for Empty<D, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Empty").finish()
    }
}

impl<D, E> Default for Empty<D, E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<D, E> Clone for Empty<D, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D, E> Copy for Empty<D, E> {}
