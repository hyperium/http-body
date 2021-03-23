use crate::Body;
use bytes::{Bytes, BytesMut};
use pin_project_lite::pin_project;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::AsyncRead;
use tokio_util::io::poll_read_buf;

pin_project! {
    /// Adapter that converts an [`AsyncRead`] into a [`Body`].
    ///
    /// [`AsyncRead`]: tokio::io::AsyncRead;
    #[derive(Debug, Clone)]
    pub struct AsyncReadBody<R> {
        #[pin]
        inner: R,
        buf: BytesMut,
    }
}

impl<R> AsyncReadBody<R> {
    /// Create a new `AsyncReadBody`.
    pub fn new(read: R) -> Self
    where
        R: AsyncRead,
    {
        Self::new_with_buffer_size(read, 1024)
    }

    /// Create a new `AsyncReadBody` using `capacity` as the initial capacity of the internal
    /// buffer.
    pub fn new_with_buffer_size(read: R, capacity: usize) -> Self
    where
        R: AsyncRead,
    {
        Self {
            inner: read,
            buf: BytesMut::with_capacity(capacity),
        }
    }

    /// Get a reference to the inner value.
    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    /// Get a mutable reference to the inner value.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Get a pinned mutable reference to the inner value.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut R> {
        self.project().inner
    }

    /// Consumes `self`, returning the inner value.
    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R> Body for AsyncReadBody<R>
where
    R: AsyncRead,
{
    type Data = Bytes;
    type Error = io::Error;

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        let mut this = self.project();

        this.buf.clear();

        let bytes_read = match poll_read_buf(this.inner, cx, &mut this.buf) {
            Poll::Ready(bytes_read) => bytes_read?,
            Poll::Pending => return Poll::Pending,
        };

        if bytes_read == 0 {
            Poll::Ready(None)
        } else {
            Poll::Ready(Some(Ok(this.buf.clone().freeze())))
        }
    }

    #[inline]
    fn poll_trailers(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Option<http::HeaderMap>, Self::Error>> {
        Poll::Ready(Ok(None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn works() {
        let read = tokio::fs::File::open("Cargo.toml").await.unwrap();
        let body = AsyncReadBody::new(read);

        let bytes = to_bytes(body).await.unwrap();
        let s = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(s.contains("name = \"http-body\""));
    }

    async fn to_bytes<B>(mut body: B) -> Result<Bytes, B::Error>
    where
        B: Body<Data = Bytes> + Unpin,
    {
        let mut buf = BytesMut::new();

        loop {
            let chunk = body.data().await.transpose()?;

            if let Some(chunk) = chunk {
                buf.extend(&chunk[..]);
            } else {
                return Ok(buf.freeze());
            }
        }
    }
}
