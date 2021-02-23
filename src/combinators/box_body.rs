use crate::Body;
use bytes::Buf;
use std::{
    fmt,
    pin::Pin,
    task::{Context, Poll},
};

/// A boxed [`Body`] trait object.
pub struct BoxBody<D> {
    inner: Pin<
        Box<
            dyn Body<Data = D, Error = Box<dyn std::error::Error + Send + Sync>>
                + Send
                + Sync
                + 'static,
        >,
    >,
}

impl<D> BoxBody<D> {
    /// Create a new `BoxBody`.
    pub fn new<B>(body: B) -> Self
    where
        B: Body<Data = D> + Send + Sync + 'static,
        B::Error: std::error::Error + Send + Sync,
        D: Buf,
    {
        Self {
            inner: Box::pin(body.map_err(|err| Box::new(err) as _)),
        }
    }
}

impl<D> fmt::Debug for BoxBody<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BoxBody")
    }
}

impl<D> Body for BoxBody<D>
where
    D: Buf,
{
    type Data = D;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn poll_data(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        self.inner.as_mut().poll_data(cx)
    }

    fn poll_trailers(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<http::HeaderMap>, Self::Error>> {
        self.inner.as_mut().poll_trailers(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> crate::SizeHint {
        self.inner.size_hint()
    }
}
