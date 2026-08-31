use bytes::Buf;
use http_body::{Body, Frame, SizeHint};
use std::{
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

/// A [`Body`] that returns an error when polled.
///
/// An [`ErrorBody<D, E>`] is a body that will yield an `E`-typed error when [`Body::poll_frame()`]
/// is called. The `D` generic allows this body to be treated as a body that would yield a
/// particular [`Body::Data`] were it not to fail.
///
/// This is most often useful for situations like exercising error-handling logic in tests.
#[derive(Debug)]
pub struct ErrorBody<D, E> {
    error: Option<E>,
    data: PhantomData<D>,
}

// === impl ErrorBody ===

impl<D, E> ErrorBody<D, E> {
    /// Returns a new [`ErrorBody`] that will yield the provided error.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    /// use http_body_util::{BodyExt, ErrorBody};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut body = ErrorBody::<Bytes, &str>::new("problem");
    ///     let frame = body.frame().await;
    ///     assert_eq!(
    ///         frame.unwrap().unwrap_err(),
    ///         "problem",
    ///     );
    /// }
    /// ```
    pub fn new(error: E) -> Self {
        Self {
            error: Some(error),
            data: PhantomData,
        }
    }
}

impl<D, E> Body for ErrorBody<D, E>
where
    E: Unpin,
    D: Buf + Unpin,
{
    type Data = D;
    type Error = E;

    fn poll_frame(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let Self { error, .. } = self.get_mut();

        if let Some(error) = error.take() {
            return Poll::Ready(Some(Err(error)));
        }

        Poll::Ready(None)
    }

    fn is_end_stream(&self) -> bool {
        self.error.is_none()
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::default()
    }
}

#[cfg(test)]
mod error_body_tests {
    use super::ErrorBody;
    use bytes::Bytes;
    use http_body::Body;
    use std::{
        ops::Not,
        pin::Pin,
        task::{Context, Poll},
    };

    #[test]
    fn returns_error() {
        type Error = &'static str;

        let mut body = ErrorBody::<Bytes, Error>::new("problem");

        assert!(
            body.is_end_stream().not(),
            "body is not finished until polled"
        );
        assert_eq!(body.size_hint().lower(), 0);
        assert_eq!(body.size_hint().upper(), None);

        let waker = futures_util::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(Some(Err("problem"))) => {}
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        assert!(body.is_end_stream(), "body is finished after being polled");

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(None) => {}
            other => panic!("unexpected poll outcome: {:?}", other),
        }
    }
}
