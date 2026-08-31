use http_body::{Body, SizeHint};
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pin_project! {
    /// A [`Body`] backed by a fallible [`Future`].
    ///
    /// This allows an `F`-typed future that will yield either a `B`-typed body, or an error, to be
    /// polled as a body. This is particularly useful when you create a body through an asynchronous
    /// computation of some sort.
    ///
    /// For example, sending a body over a oneshot channel or reading its contents from the filesystem.
    #[project = TryFutureBodyProj]
    pub struct TryFutureBody<F, B> {
        #[pin]
        inner: Inner<F, B>,
    }
}

pin_project! {
    /// The inner state of a [`TryFutureBody<F, B>`].
    ///
    /// A future is polled until it either yields a body, or fails.
    ///
    /// ```text
    /// ┌────────┐                                               ┌──────┐
    /// │ Future │ --> `poll_frame()`-+------------------------> │ Body │
    /// └────────┘                    | `Poll::Ready(Ok(body))`  └──────┘
    ///     ↑               |         |
    ///     |               |         |                          ┌────────┐
    ///     +---------------+         +------------------------> │ Failed │
    ///      `Poll::Pending`            `Poll::Ready(Err(err))`  └────────┘
    ///
    /// ```
    #[project = InnerProj]
    enum Inner<F, B> {
        /// The future is still being polled.
        ///
        /// When the body is in this state, the inner future has not yet resolved. When this body is
        /// polled, this inner future will be polled.
        Future { #[pin] future: F },
        /// The body has been yielded and is being polled.
        ///
        /// When the body is in this state, the future has already yielded a body that can now be read.
        Body { #[pin] body: B },
        /// The future failed to yield a body.
        Failed,
    }
}

// === impl TryFutureBody ===

impl<F, B> TryFutureBody<F, B> {
    /// Wraps the provided future in a [`TryFutureBody<F, B>`].
    pub fn new(future: F) -> Self {
        Self {
            inner: Inner::Future { future },
        }
    }
}

impl<F, B, E> Body for TryFutureBody<F, B>
where
    F: Future<Output = Result<B, E>>,
    B: http_body::Body,
    E: Into<B::Error>,
{
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        let TryFutureBodyProj { inner } = self.as_mut().project();
        match inner.project() {
            InnerProj::Failed => Poll::Ready(None),
            InnerProj::Body { body } => body.poll_frame(cx),
            InnerProj::Future { future } => match future.poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Ok(body)) => {
                    // We received the body. Put it into place, and then poll ourselves again.
                    let inner = Inner::Body { body };
                    self.set(Self { inner });
                    self.poll_frame(cx)
                }
                Poll::Ready(Err(err)) => {
                    // There is no body. Mark ourselves as finished and return the error.
                    let inner = Inner::Failed;
                    self.set(Self { inner });
                    Poll::Ready(Some(Err(err.into())))
                }
            },
        }
    }

    fn is_end_stream(&self) -> bool {
        let Self { inner } = self;
        match inner {
            Inner::Future { .. } => false,
            Inner::Body { body } => body.is_end_stream(),
            Inner::Failed => true,
        }
    }

    fn size_hint(&self) -> SizeHint {
        let Self { inner } = self;
        match inner {
            Inner::Future { .. } => SizeHint::new(),
            Inner::Body { body } => body.size_hint(),
            Inner::Failed => SizeHint::with_exact(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Full;
    use bytes::Bytes;
    use std::{convert::Infallible, future::ready, ops::Not};

    #[test]
    fn full_ready_body() {
        let mut body = {
            let data = Bytes::from_static(b"hello");
            let body = Full::<Bytes>::from(data);
            let fut = ready(Ok::<_, Infallible>(body));
            TryFutureBody::new(fut)
        };

        // Confirm that hints are correct before we poll the future.
        {
            assert!(
                body.is_end_stream().not(),
                "stream is not over before future resolves"
            );
            let hint = body.size_hint();
            assert_eq!(
                hint.lower(),
                0,
                "size hint has lower bound of 0 before future resolves"
            );
            assert_eq!(
                hint.upper(),
                None,
                "size hint has no upper bound before future resolves"
            );
        }

        let waker = futures_util::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        // Now poll the body. The future will resolve, and the underlying body will yield "hello".
        {
            let body = Pin::new(&mut body);
            let Poll::Ready(Some(Ok(frame))) = body.poll_frame(&mut cx) else {
                panic!("body should yield a frame when polled");
            };
            let data = frame.into_data().expect("frame should contain data");
            assert_eq!(data, "hello", "underlying body frames are returned");
        }

        // The body will yield None after the inner body has finished.
        {
            let body = Pin::new(&mut body);
            let Poll::Ready(None) = body.poll_frame(&mut cx) else {
                panic!("body should `Ready(None)` when polled");
            };
        }

        // Finally, show that the body properly reports that the stream is finished.
        {
            assert!(
                body.is_end_stream(),
                "stream is over after body is finished"
            );
            let hint = body.size_hint();
            assert_eq!(
                hint.upper(),
                Some(0),
                "size hint is upper bound of 0 after body is finished"
            );
        }
    }

    /// A [`Body`] that returns an `E`-typed error when polled.
    struct ErrorBody<E = String> {
        error: Option<E>,
    }

    impl<E> ErrorBody<E> {
        fn new(error: E) -> Self {
            Self { error: Some(error) }
        }
    }

    impl<E: Unpin> Body for ErrorBody<E> {
        type Data = Bytes;
        type Error = E;

        fn poll_frame(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
            let Self { error } = self.get_mut();
            let error = error.take().map(Err);
            Poll::Ready(error)
        }

        fn is_end_stream(&self) -> bool {
            self.error.is_none()
        }

        fn size_hint(&self) -> SizeHint {
            if self.error.is_some() {
                // Pretend there is a hint until the error is returned.
                SizeHint::with_exact(42)
            } else {
                SizeHint::with_exact(0)
            }
        }
    }

    /// Show that a body that returns an error will be processed correctly.
    #[test]
    fn error_body() {
        type Error = &'static str;
        const ERROR: Error = "houston we have a problem";

        let mut body = {
            let body = ErrorBody::new(ERROR);
            let fut = ready(Ok::<_, Error>(body));
            TryFutureBody::new(fut)
        };

        // Confirm that hints are correct before we poll the future.
        {
            assert!(
                body.is_end_stream().not(),
                "stream is not over before future resolves"
            );
            let hint = body.size_hint();
            assert_eq!(
                hint.lower(),
                0,
                "size hint has lower bound of 0 before future resolves"
            );
            assert_eq!(
                hint.upper(),
                None,
                "size hint has no upper bound before future resolves"
            );
        }

        // Now poll the body. The future will resolve, and the underlying body will yield "hello".
        {
            let body = Pin::new(&mut body);
            let waker = futures_util::task::noop_waker();
            let mut cx = Context::from_waker(&waker);
            let Poll::Ready(Some(Err(err))) = body.poll_frame(&mut cx) else {
                panic!("body should yield an error when polled");
            };
            assert_eq!(err, ERROR, "future errors are returned");
        }

        // Finally, show that the body properly reports that the stream is finished.
        {
            assert!(
                body.is_end_stream(),
                "stream is over after body is finished"
            );
            let hint = body.size_hint();
            assert_eq!(
                hint.upper(),
                Some(0),
                "size hint is upper bound of 0 after body is finished"
            );
        }
    }

    /// Show that a future that fails to yield a body will be processed correctly.
    #[test]
    fn error_future() {
        const ERROR: &str = "there is no body";

        let mut body = {
            let fut = ready(Err::<ErrorBody, _>(ERROR.to_string()));
            TryFutureBody::new(fut)
        };

        // Confirm that hints are correct before we poll the future.
        {
            assert!(
                body.is_end_stream().not(),
                "stream is not over before future resolves"
            );
            let hint = body.size_hint();
            assert_eq!(
                hint.lower(),
                0,
                "size hint has lower bound of 0 before future resolves"
            );
            assert_eq!(
                hint.upper(),
                None,
                "size hint has no upper bound before future resolves"
            );
        }

        // Now poll the body. The future will resolve, and the underlying body will yield "hello".
        {
            let body = Pin::new(&mut body);
            let waker = futures_util::task::noop_waker();
            let mut cx = Context::from_waker(&waker);
            let Poll::Ready(Some(Err(err))) = body.poll_frame(&mut cx) else {
                panic!("body should yield an error when polled");
            };
            assert_eq!(err, "there is no body", "future errors are returned");
        }

        // Finally, show that the body properly reports that the stream is finished.
        {
            assert!(
                body.is_end_stream(),
                "stream is over after body is finished"
            );
            let hint = body.size_hint();
            assert_eq!(
                hint.upper(),
                Some(0),
                "size hint is upper bound of 0 after body is finished"
            );
        }
    }
}
