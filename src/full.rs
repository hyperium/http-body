use crate::{Body, SizeHint};
use bytes::{Buf, Bytes};
use http::HeaderMap;
use pin_project_lite::pin_project;
use std::borrow::Cow;
use std::convert::{Infallible, TryFrom};
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    /// A body that consists of a single chunk.
    #[derive(Clone, Copy, Debug)]
    pub struct Full<D, E = Infallible> {
        data: Option<D>,
        _error: PhantomData<fn() -> E>,
    }
}

impl<D, E> Full<D, E>
where
    D: Buf,
{
    /// Create a new `Full`.
    pub fn new(data: D) -> Self {
        let data = if data.has_remaining() {
            Some(data)
        } else {
            None
        };
        Full {
            data,
            _error: PhantomData,
        }
    }
}

impl<D, E> Body for Full<D, E>
where
    D: Buf,
{
    type Data = D;
    type Error = E;

    fn poll_data(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<D, Self::Error>>> {
        Poll::Ready(self.data.take().map(Ok))
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        Poll::Ready(Ok(None))
    }

    fn is_end_stream(&self) -> bool {
        self.data.is_none()
    }

    fn size_hint(&self) -> SizeHint {
        self.data
            .as_ref()
            .map(|data| SizeHint::with_exact(u64::try_from(data.remaining()).unwrap()))
            .unwrap_or_else(|| SizeHint::with_exact(0))
    }
}

impl<D, E> Default for Full<D, E>
where
    D: Buf,
{
    /// Create an empty `Full`.
    fn default() -> Self {
        Full {
            data: None,
            _error: PhantomData,
        }
    }
}

impl<D, E> From<Bytes> for Full<D, E>
where
    D: Buf + From<Bytes>,
{
    fn from(bytes: Bytes) -> Self {
        Full::new(D::from(bytes))
    }
}

impl<D, E> From<Vec<u8>> for Full<D, E>
where
    D: Buf + From<Vec<u8>>,
{
    fn from(vec: Vec<u8>) -> Self {
        Full::new(D::from(vec))
    }
}

impl<D, E> From<&'static [u8]> for Full<D, E>
where
    D: Buf + From<&'static [u8]>,
{
    fn from(slice: &'static [u8]) -> Self {
        Full::new(D::from(slice))
    }
}

impl<D, E, B> From<Cow<'static, B>> for Full<D, E>
where
    D: Buf + From<&'static B> + From<B::Owned>,
    B: ToOwned + ?Sized,
{
    fn from(cow: Cow<'static, B>) -> Self {
        match cow {
            Cow::Borrowed(b) => Full::new(D::from(b)),
            Cow::Owned(o) => Full::new(D::from(o)),
        }
    }
}

impl<D, E> From<String> for Full<D, E>
where
    D: Buf + From<String>,
{
    fn from(s: String) -> Self {
        Full::new(D::from(s))
    }
}

impl<D, E> From<&'static str> for Full<D, E>
where
    D: Buf + From<&'static str>,
{
    fn from(slice: &'static str) -> Self {
        Full::new(D::from(slice))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn full_returns_some() {
        let mut full = Full::new(&b"hello"[..]);
        assert_eq!(full.size_hint().exact(), Some(b"hello".len() as u64));
        assert_eq!(full.data().await, Some(Ok::<_, Infallible>(&b"hello"[..])));
        assert!(full.data().await.is_none());
    }

    #[tokio::test]
    async fn empty_full_returns_none() {
        assert!(Full::<&[u8]>::default().data().await.is_none());
        assert!(Full::<_, Infallible>::new(&b""[..]).data().await.is_none());
    }
}
