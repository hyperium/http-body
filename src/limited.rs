//! Body types.

use crate::{Body, SizeHint};
use bytes::Buf;
use pin_project_lite::pin_project;
use std::{
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
                    Some(Err("length limit exceeded".into()))
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
        self.project().inner.poll_trailers(cx).map_err(|e| e.into())
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

#[cfg(test)]
mod tests {
    use super::Limited;
    use crate::{Body, Full};
    use bytes::{BufMut, Bytes, BytesMut};

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

    async fn to_bytes<B: Body>(body: B) -> Result<Bytes, B::Error> {
        tokio::pin!(body);

        let mut bytes = BytesMut::new();
        while let Some(result) = body.data().await {
            bytes.put(result?);
        }

        Ok(bytes.freeze())
    }
}
