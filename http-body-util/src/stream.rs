use bytes::Buf;
use futures_util::stream::Stream;
use http::HeaderMap;
use http_body::Body;
use pin_project_lite::pin_project;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

pin_project! {
    /// A body created from a `Stream`.
    #[derive(Debug)]
    pub struct StreamBody<S> {
        #[pin]
        stream: S,
    }
}

impl<S> StreamBody<S> {
    /// Create a new `StreamBody`.
    pub fn new(stream: S) -> Self {
        Self { stream }
    }
}

impl<S, D, E> Body for StreamBody<S>
where
    S: Stream<Item = Result<D, E>>,
    D: Buf,
{
    type Data = D;
    type Error = E;

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        self.project().stream.poll_next(cx)
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        Poll::Ready(Ok(None))
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
