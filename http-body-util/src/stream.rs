use bytes::Buf;
use futures_util::stream::{BoxStream, Stream};
use http::HeaderMap;
use http_body::Body;
use std::{
    fmt,
    pin::Pin,
    task::{Context, Poll},
};

/// A body created from a `Stream`.
pub struct StreamBody<D, E> {
    stream: BoxStream<'static, Result<D, E>>,
}

impl<D, E> StreamBody<D, E> {
    /// Create a new `StreamBody`.
    pub fn new<S>(stream: S) -> Self
    where
        S: Stream<Item = Result<D, E>> + Send + 'static,
    {
        Self {
            stream: Box::pin(stream),
        }
    }
}

impl<D: Buf, E> Body for StreamBody<D, E> {
    type Data = D;
    type Error = E;

    fn poll_data(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        self.stream.as_mut().poll_next(cx)
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        Poll::Ready(Ok(None))
    }
}

impl<D, E> fmt::Debug for StreamBody<D, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamBody").finish()
    }
}

#[cfg(test)]
mod tests {
    use crate::StreamBody;
    use bytes::Bytes;
    use http_body::Body;
    use std::convert::Infallible;

    #[tokio::test]
    async fn body_from_stream() {
        let chunks: Vec<Result<Bytes, Infallible>> = vec![
            Ok(Bytes::from(vec![1])),
            Ok(Bytes::from(vec![2])),
            Ok(Bytes::from(vec![3])),
        ];
        let stream = futures_util::stream::iter(chunks);
        let mut body = StreamBody::new(stream);

        assert_eq!(body.data().await.unwrap().unwrap().as_ref(), [1]);
        assert_eq!(body.data().await.unwrap().unwrap().as_ref(), [2]);
        assert_eq!(body.data().await.unwrap().unwrap().as_ref(), [3]);

        assert!(body.data().await.is_none());
    }
}
