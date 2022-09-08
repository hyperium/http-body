use bytes::Buf;
use futures_util::stream::Stream;
use http_body::{Body, Frame};
use pin_project_lite::pin_project;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

pin_project! {
    /// A body created from a `Stream`.
    #[derive(Clone, Copy, Debug)]
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
    S: Stream<Item = Result<Frame<D>, E>>,
    D: Buf,
{
    type Data = D;
    type Error = E;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match self.project().stream.poll_next(cx) {
            Poll::Ready(Some(result)) => Poll::Ready(Some(result)),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S: Stream> Stream for StreamBody<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().stream.poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

#[cfg(test)]
mod tests {
    use crate::{BodyExt, StreamBody};
    use bytes::Bytes;
    use http_body::Frame;
    use std::convert::Infallible;

    #[tokio::test]
    async fn body_from_stream() {
        let chunks: Vec<Result<_, Infallible>> = vec![
            Ok(Frame::data(Bytes::from(vec![1]))),
            Ok(Frame::data(Bytes::from(vec![2]))),
            Ok(Frame::data(Bytes::from(vec![3]))),
        ];
        let stream = futures_util::stream::iter(chunks);
        let mut body = StreamBody::new(stream);

        assert_eq!(
            body.frame()
                .await
                .unwrap()
                .unwrap()
                .into_data()
                .unwrap()
                .as_ref(),
            [1]
        );
        assert_eq!(
            body.frame()
                .await
                .unwrap()
                .unwrap()
                .into_data()
                .unwrap()
                .as_ref(),
            [2]
        );
        assert_eq!(
            body.frame()
                .await
                .unwrap()
                .unwrap()
                .into_data()
                .unwrap()
                .as_ref(),
            [3]
        );

        assert!(body.frame().await.is_none());
    }
}
