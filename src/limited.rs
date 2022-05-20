//! Body types.

use crate::{Body, SizeHint};
use bytes::Buf;
use pin_project_lite::pin_project;
use std::{
    fmt,
    pin::Pin,
    task::{Context, Poll},
};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

pin_project! {
    /// Body wrapper that returns error when limit is exceeded.
    #[derive(Clone, Copy, Debug)]
    pub struct Limited<B> {
        #[pin]
        inner: B,
        remaining: usize,
    }
}

impl<B> Limited<B> {
    /// Crate a new [`Limited`].
    pub fn new(inner: B, limit: usize) -> Self {
        Self {
            inner,
            remaining: limit,
        }
    }
}

impl<B> Body for Limited<B>
where
    B: Body,
    B::Error: Into<BoxError>,
{
    type Data = B::Data;
    type Error = BoxError;

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        let this = self.project();

        let res = match this.inner.poll_data(cx) {
            Poll::Ready(Some(Ok(data))) => {
                if data.remaining() > *this.remaining {
                    *this.remaining = 0;
                    Some(Err(LengthLimitError::new().into()))
                } else {
                    *this.remaining -= data.remaining();
                    Some(Ok(data))
                }
            }
            Poll::Ready(Some(Err(e))) => Some(Err(e.into())),
            Poll::Ready(None) => None,
            Poll::Pending => return Poll::Pending,
        };

        Poll::Ready(res)
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<http::HeaderMap>, Self::Error>> {
        self.project().inner.poll_trailers(cx).map_err(Into::into)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        use std::convert::TryFrom;
        match u64::try_from(self.remaining) {
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
pub struct LengthLimitError {}

impl LengthLimitError {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl fmt::Display for LengthLimitError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("length limit exceeded")
    }
}

impl std::error::Error for LengthLimitError {}

#[cfg(test)]
mod tests {
    use super::Limited;
    use crate::{Body, Full, SizeHint};
    use bytes::{BufMut, Bytes, BytesMut};
    use std::{
        convert::Infallible,
        pin::Pin,
        task::{Context, Poll},
    };

    #[tokio::test]
    async fn over_limit() {
        let body = Full::new(Bytes::from(vec![0u8; 4096]));
        let limited_body = Limited::new(body, 2048);

        assert!(to_bytes(limited_body).await.is_err());
    }

    #[tokio::test]
    async fn under_limit() {
        let body = Full::new(Bytes::from(vec![0u8; 4096]));
        let limited_body = Limited::new(body, 8192);

        assert!(to_bytes(limited_body).await.is_ok());
    }

    #[tokio::test]
    async fn size_hint() {
        const CHUNK: [u8; 8] = [0u8; 8];

        enum TestBody {
            Empty,
            Half,
            Full,
        }

        impl Body for TestBody {
            type Data = Bytes;
            type Error = Infallible;

            fn poll_data(
                mut self: Pin<&mut Self>,
                _cx: &mut Context<'_>,
            ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
                match *self {
                    Self::Empty => self.set(Self::Half),
                    Self::Half => self.set(Self::Full),
                    Self::Full => return Poll::Ready(None),
                }

                Poll::Ready(Some(Ok(CHUNK.to_vec().into())))
            }

            fn poll_trailers(
                self: Pin<&mut Self>,
                _cx: &mut Context<'_>,
            ) -> Poll<Result<Option<http::HeaderMap>, Self::Error>> {
                unimplemented!()
            }

            fn size_hint(&self) -> SizeHint {
                match self {
                    Self::Empty => SizeHint::with_exact((CHUNK.len() * 2) as u64),
                    Self::Half => SizeHint::with_exact(CHUNK.len() as u64),
                    Self::Full => SizeHint::with_exact(0),
                }
            }
        }

        let mut body = TestBody::Empty;

        assert_eq!(body.size_hint().upper().unwrap(), (CHUNK.len() * 2) as u64);

        let data = body.data().await.unwrap().unwrap();
        assert_eq!(data, CHUNK.to_vec());

        assert_eq!(body.size_hint().upper().unwrap(), CHUNK.len() as u64);

        let data = body.data().await.unwrap().unwrap();
        assert_eq!(data, CHUNK.to_vec());

        assert_eq!(body.size_hint().upper().unwrap(), 0);
    }

    async fn to_bytes<B: Body>(body: B) -> Result<Bytes, B::Error> {
        tokio::pin!(body);

        let mut bytes = BytesMut::new();
        while let Some(result) = body.data().await {
            bytes.put(result?);
        }

        Ok(bytes.freeze())
    }
}
