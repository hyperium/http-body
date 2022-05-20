use crate::{Body, SizeHint};
use bytes::Buf;
use http::HeaderMap;
use std::error::Error;
use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};

/// A length limited body.
///
/// This body will return an error if more than `N` bytes are returned
/// on polling the wrapped body.
#[derive(Clone, Copy, Debug)]
pub struct Limited<B, const N: usize> {
    remaining: usize,
    inner: B,
}

impl<B> Limited<B, 0> {
    /// Create a new `Limited`.
    pub fn new<const N: usize>(inner: B) -> Limited<B, N> {
        Limited {
            remaining: N,
            inner,
        }
    }
}

impl<B, const N: usize> Default for Limited<B, N>
where
    B: Default,
{
    fn default() -> Self {
        Limited::new(B::default())
    }
}

impl<B, const N: usize> Body for Limited<B, N>
where
    B: Body + Unpin,
{
    type Data = B::Data;
    type Error = LengthLimitError<B::Error>;

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        let mut this = self;
        let res = match Pin::new(&mut this.inner).poll_data(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(None) => None,
            Poll::Ready(Some(Ok(data))) => {
                if data.remaining() > this.remaining {
                    this.remaining = 0;
                    Some(Err(LengthLimitError::LengthLimitExceeded))
                } else {
                    this.remaining -= data.remaining();
                    Some(Ok(data))
                }
            }
            Poll::Ready(Some(Err(err))) => Some(Err(LengthLimitError::Other(err))),
        };

        Poll::Ready(res)
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        let mut this = self;
        let res = match Pin::new(&mut this.inner).poll_trailers(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(Ok(data)) => Ok(data),
            Poll::Ready(Err(err)) => Err(LengthLimitError::Other(err)),
        };

        Poll::Ready(res)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        use std::convert::TryFrom;
        match u64::try_from(N) {
            Ok(n) => {
                let mut hint = self.inner.size_hint();
                if hint.lower() >= n {
                    hint.set_exact(n)
                } else if let Some(max) = hint.upper() {
                    hint.set_upper(n.min(max))
                } else {
                    hint.set_upper(n)
                }
                hint
            }
            Err(_) => self.inner.size_hint(),
        }
    }
}

/// An error returned when reading from a [`Limited`] body.
#[derive(Debug)]
pub enum LengthLimitError<E> {
    /// The body exceeded the length limit.
    LengthLimitExceeded,
    /// Some other error was encountered while reading from the underlying body.
    Other(E),
}

impl<E> fmt::Display for LengthLimitError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::LengthLimitExceeded => f.write_str("length limit exceeded"),
            Self::Other(err) => err.fmt(f),
        }
    }
}

impl<E> Error for LengthLimitError<E>
where
    E: Error,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::LengthLimitExceeded => None,
            Self::Other(err) => err.source(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Full;
    use bytes::Bytes;
    use std::convert::Infallible;

    #[tokio::test]
    async fn read_for_body_under_limit_returns_data() {
        const DATA: &[u8] = b"testing";
        let inner = Full::new(Bytes::from(DATA));
        let body = &mut Limited::new::<8>(inner);
        let data = body.data().await.unwrap().unwrap();
        assert_eq!(data, DATA);
        assert!(matches!(body.data().await, None));
    }

    #[tokio::test]
    async fn read_for_body_over_limit_returns_error() {
        const DATA: &[u8] = b"testing a string that is too long";
        let inner = Full::new(Bytes::from(DATA));
        let body = &mut Limited::new::<8>(inner);
        let error = body.data().await.unwrap().unwrap_err();
        assert!(matches!(error, LengthLimitError::LengthLimitExceeded));
    }

    struct Chunky(&'static [&'static [u8]]);

    impl Body for Chunky {
        type Data = &'static [u8];
        type Error = Infallible;

        fn poll_data(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
            let mut this = self;
            match this.0.split_first().map(|(&head, tail)| (Ok(head), tail)) {
                Some((data, new_tail)) => {
                    this.0 = new_tail;

                    Poll::Ready(Some(data))
                }
                None => Poll::Ready(None),
            }
        }

        fn poll_trailers(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
            Poll::Ready(Ok(Some(HeaderMap::new())))
        }
    }

    #[tokio::test]
    async fn read_for_chunked_body_around_limit_returns_first_chunk_but_returns_error_on_over_limit_chunk(
    ) {
        const DATA: &[&[u8]] = &[b"testing ", b"a string that is too long"];
        let inner = Chunky(DATA);
        let body = &mut Limited::new::<8>(inner);
        let data = body.data().await.unwrap().unwrap();
        assert_eq!(data, DATA[0]);
        let error = body.data().await.unwrap().unwrap_err();
        assert!(matches!(error, LengthLimitError::LengthLimitExceeded));
    }

    #[tokio::test]
    async fn read_for_chunked_body_over_limit_on_first_chunk_returns_error() {
        const DATA: &[&[u8]] = &[b"testing a string", b" that is too long"];
        let inner = Chunky(DATA);
        let body = &mut Limited::new::<8>(inner);
        let error = body.data().await.unwrap().unwrap_err();
        assert!(matches!(error, LengthLimitError::LengthLimitExceeded));
    }

    #[tokio::test]
    async fn read_for_chunked_body_under_limit_is_okay() {
        const DATA: &[&[u8]] = &[b"test", b"ing!"];
        let inner = Chunky(DATA);
        let body = &mut Limited::new::<8>(inner);
        let data = body.data().await.unwrap().unwrap();
        assert_eq!(data, DATA[0]);
        let data = body.data().await.unwrap().unwrap();
        assert_eq!(data, DATA[1]);
        assert!(matches!(body.data().await, None));
    }

    #[tokio::test]
    async fn read_for_trailers_propagates_inner_trailers() {
        const DATA: &[&[u8]] = &[b"test", b"ing!"];
        let inner = Chunky(DATA);
        let body = &mut Limited::new::<8>(inner);
        let trailers = body.trailers().await.unwrap();
        assert_eq!(trailers, Some(HeaderMap::new()))
    }

    enum ErrorBodyError {
        Data,
        Trailers,
    }

    struct ErrorBody;

    impl Body for ErrorBody {
        type Data = &'static [u8];
        type Error = ErrorBodyError;

        fn poll_data(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
            Poll::Ready(Some(Err(ErrorBodyError::Data)))
        }

        fn poll_trailers(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
            Poll::Ready(Err(ErrorBodyError::Trailers))
        }
    }

    #[tokio::test]
    async fn read_for_body_returning_error_propagates_error() {
        let body = &mut Limited::new::<8>(ErrorBody);
        let error = body.data().await.unwrap().unwrap_err();
        assert!(matches!(
            error,
            LengthLimitError::Other(ErrorBodyError::Data)
        ));
    }

    #[tokio::test]
    async fn trailers_for_body_returning_error_propagates_error() {
        let body = &mut Limited::new::<8>(ErrorBody);
        let error = body.trailers().await.unwrap_err();
        assert!(matches!(
            error,
            LengthLimitError::Other(ErrorBodyError::Trailers)
        ));
    }
}
