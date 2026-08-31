use http::HeaderMap;
use http_body::{Body, Frame, SizeHint};
use pin_project_lite::pin_project;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

pin_project! {
    /// A body that links two bodies together, in a chain.
    ///
    /// See [`BodyExt::chain()`] for more information.
    #[project = ChainProj]
    pub struct Chain<A, B> {
        #[pin]
        inner: Inner<A, B>,
    }
}

pin_project! {
    #[project = InnerProj]
    pub enum Inner<A, B> {
        First {
            #[pin]
            first: A,
            second: Option<B>,
        },
        Second {
            #[pin]
            second: B,
            trailers: Option<http::HeaderMap>,
        },
        Finished,
    }
}

// === impl Chain ===

impl<A, B> Chain<A, B> {
    /// Returns a "chained" body.
    ///
    /// The contents of the first provided body will precede the contents of the second body.
    pub fn new(first: A, second: B) -> Self {
        Self {
            inner: Inner::First {
                first,
                second: Some(second),
            },
        }
    }
}

impl<A, B> Body for Chain<A, B>
where
    A: Body<Data = B::Data>,
    B: Body,
    A::Error: Into<B::Error>,
{
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let ChainProj { inner } = self.as_mut().project();
        match inner.project() {
            InnerProj::First { first, second } => {
                // Poll the first body until it yields a frame.
                let frame = match first.poll_frame(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(res) => res,
                };

                let trailers: Option<HeaderMap> = match frame {
                    // There are no more frames in the first body.
                    None => None,
                    // The first body has yielded a frame.
                    Some(Ok(frame)) => {
                        match frame.into_trailers() {
                            // A `TRAILERS` frame was yielded...
                            Ok(trls) => Some(trls),
                            // We will return other kinds of frames, and continue to poll the
                            // first body next time around.
                            Err(frame) => return Poll::Ready(Some(Ok(frame))),
                        }
                    }
                    Some(Err(err)) => {
                        // The first body returned an error! We are now finished.
                        let inner = Inner::Finished;
                        self.set(Self { inner });
                        return Poll::Ready(Some(Err(err.into())));
                    }
                };

                // If we are here, the first body is complete. Prepare to poll the second body.
                let second = second.take().unwrap();
                let inner = Inner::Second { second, trailers };
                self.set(Self { inner });
                self.poll_frame(cx)
            }
            InnerProj::Second {
                second,
                trailers: trailers_first,
            } => {
                // Poll the second body until it yields a frame.
                let frame = match second.poll_frame(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(res) => res,
                };

                let trailers_second: Option<HeaderMap> = match frame {
                    // There are no more frames in the second body.
                    None => None,
                    // The second body has yielded a frame.
                    Some(Ok(frame)) => match frame.into_trailers() {
                        // A `TRAILERS` frame was yielded...
                        Ok(trls) => Some(trls),
                        // We will return other kinds of frames, and continue to poll the
                        // second body next time around.
                        Err(frame) => return Poll::Ready(Some(Ok(frame))),
                    },
                    Some(Err(err)) => {
                        // The second body returned an error! We are now finished.
                        let inner = Inner::Finished;
                        self.set(Self { inner });
                        return Poll::Ready(Some(Err(err)));
                    }
                };

                // If we are here, the second body is complete. We should now return trailers
                // that have been accumulated from the two bodies.
                let trailers = match (trailers_first.take(), trailers_second) {
                    (Some(mut a), Some(b)) => {
                        a.extend(b);
                        Some(Ok(Frame::trailers(a)))
                    }
                    (Some(t), None) | (None, Some(t)) => Some(Ok(Frame::trailers(t))),
                    (None, None) => None,
                };

                // Mark the body as finished before returning the trailers.
                let inner = Inner::Finished;
                self.set(Self { inner });
                Poll::Ready(trailers)
            }
            InnerProj::Finished => Poll::Ready(None),
        }
    }

    fn is_end_stream(&self) -> bool {
        let Self { inner } = self;
        match inner {
            Inner::First { .. } | Inner::Second { .. } => false,
            Inner::Finished => true,
        }
    }

    fn size_hint(&self) -> SizeHint {
        let Self { inner } = self;
        match inner {
            // If the first body is still being polled, return the sum of the two bodies' hints.
            Inner::First { first, second } => {
                let first = first.size_hint();
                let second = second.as_ref().map(Body::size_hint).unwrap_or_default();
                first + second
            }
            // If the second body is now being polled, forward its hint.
            Inner::Second { second, .. } => second.size_hint(),
            // If this body is finished, return a hint of 0.
            Inner::Finished => {
                let mut hint = SizeHint::new();
                hint.set_exact(0);
                hint
            }
        }
    }
}

/// Unit tests for [`Chain<A, B>`].
#[cfg(test)]
mod chain_tests {
    use crate::{BodyExt, Empty, Full};
    use bytes::Bytes;
    use http::{HeaderMap, HeaderName, HeaderValue};
    use http_body::{Body, Frame};
    use std::{
        future::ready,
        ops::Not,
        pin::Pin,
        task::{Context, Poll},
    };

    #[tokio::test]
    async fn two_empty() {
        let mut body = {
            type Data = Bytes;
            let first = Empty::<Data>::new();
            let second = Empty::<Data>::new();
            first.chain(second)
        };

        assert!(
            body.is_end_stream().not(),
            "body is not finished until polled"
        );
        assert_eq!(body.size_hint().lower(), 0, "empty bodies size hint is 0");
        assert_eq!(
            body.size_hint().upper(),
            Some(0),
            "empty bodies size hint is 0"
        );

        let waker = futures_util::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(None) => {}
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        assert!(body.is_end_stream(), "body is finished after being polled");
    }

    #[tokio::test]
    async fn full_then_empty() {
        let mut body = {
            type Data = Bytes;
            let first = Full::new("hello ".into());
            let second = Empty::<Data>::new();
            first.chain(second)
        };

        assert!(
            body.is_end_stream().not(),
            "body is not finished until polled"
        );
        assert_eq!(body.size_hint().lower(), "hello ".len() as u64);
        assert_eq!(body.size_hint().upper(), Some("hello ".len() as u64));

        let waker = futures_util::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(Some(Ok(frame))) => {
                let frame = frame.into_data().expect("should yield data");
                assert_eq!(frame, "hello ");
            }
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        assert!(
            body.is_end_stream().not(),
            "body is not finished until second body is polled"
        );

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(None) => {}
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        assert!(body.is_end_stream(), "body is finished after being polled");
    }

    #[tokio::test]
    async fn empty_then_full() {
        let mut body = {
            type Data = Bytes;
            let first = Empty::<Data>::new();
            let second = Full::new("world!".into());
            first.chain(second)
        };

        let waker = futures_util::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        assert!(
            body.is_end_stream().not(),
            "body is not finished until polled"
        );
        assert_eq!(body.size_hint().lower(), "world!".len() as u64);
        assert_eq!(body.size_hint().upper(), Some("world!".len() as u64));

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(Some(Ok(frame))) => {
                let frame = frame.into_data().expect("should yield data");
                assert_eq!(frame, "world!");
            }
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        assert!(
            body.is_end_stream().not(),
            "body is not finished until second body is finished"
        );

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(None) => {}
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        assert!(body.is_end_stream(), "body is finished after being polled");
    }

    #[tokio::test]
    async fn two_bodies_chain() {
        let mut body = {
            type Data = Bytes;
            let first = Full::<Data>::new("hello ".into());
            let second = Full::<Data>::new("world!".into());
            first.chain(second)
        };

        let waker = futures_util::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        assert!(
            body.is_end_stream().not(),
            "body is not finished until polled"
        );
        assert_eq!(body.size_hint().lower(), "hello world!".len() as u64);
        assert_eq!(body.size_hint().upper(), Some("hello world!".len() as u64));

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(Some(Ok(frame))) => {
                let frame = frame.into_data().expect("should yield data");
                assert_eq!(frame, "hello ");
            }
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(Some(Ok(frame))) => {
                let frame = frame.into_data().expect("should yield data");
                assert_eq!(frame, "world!");
            }
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        assert!(
            body.is_end_stream().not(),
            "body is not finished until second body is finished"
        );

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(None) => {}
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        assert!(body.is_end_stream(), "body is finished after being polled");
    }

    use self::error_body::*;
    mod error_body {
        use super::*;

        /// A [`Body`] that returns an error.
        pub(super) struct ErrorBody {
            error: Option<ErrorBodyError>,
        }

        #[derive(Debug)]
        pub(super) struct ErrorBodyError(pub(super) &'static str);

        // === ErrorBody ===

        impl ErrorBody {
            pub(super) fn new(msg: &'static str) -> Self {
                Self {
                    error: Some(ErrorBodyError(msg)),
                }
            }
        }

        impl Body for ErrorBody {
            type Data = Bytes;
            type Error = ErrorBodyError;

            fn poll_frame(
                self: Pin<&mut Self>,
                _cx: &mut Context<'_>,
            ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
                let Self { error } = self.get_mut();

                Poll::Ready(error.take().map(Result::Err))
            }
        }
    }

    use self::mock_body::*;
    mod mock_body {
        //! NB: if `Full` was generic over its error, we could remove this. see hyperium/http-body#85.
        use super::*;
        use std::marker::PhantomData;

        pub(super) struct MockBody<E> {
            data: Option<Bytes>,
            _marker: PhantomData<E>,
        }

        impl<E> MockBody<E> {
            pub(super) fn new(data: impl Into<Bytes>) -> Self {
                Self {
                    data: Some(data.into()),
                    _marker: PhantomData,
                }
            }
        }

        impl<E> Body for MockBody<E>
        where
            E: Unpin,
        {
            type Data = Bytes;
            type Error = E;

            fn poll_frame(
                self: Pin<&mut Self>,
                _cx: &mut Context<'_>,
            ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
                let Self { data, _marker } = self.get_mut();
                let frame = data.take().map(Frame::data).map(Result::Ok);
                Poll::Ready(frame)
            }
        }
    }

    #[tokio::test]
    async fn second_body_error() {
        let mut body = {
            let first = MockBody::<ErrorBodyError>::new("hello ");
            let second = ErrorBody::new("failure");
            first.chain(second)
        };

        let waker = futures_util::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(Some(Ok(frame))) => {
                let frame = frame.into_data().expect("should yield data");
                assert_eq!(frame, "hello ");
            }
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(Some(Err(ErrorBodyError("failure")))) => {}
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        assert!(
            body.is_end_stream(),
            "body is finished after returning an error"
        );

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(None) => {}
            other => panic!("unexpected poll outcome: {:?}", other),
        }
    }

    #[tokio::test]
    async fn first_body_error() {
        let mut body = {
            let first = ErrorBody::new("failure");
            let second = MockBody::<ErrorBodyError>::new("world!");
            first.chain(second)
        };

        let waker = futures_util::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(Some(Err(ErrorBodyError("failure")))) => {}
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        assert!(
            body.is_end_stream(),
            "body is finished after returning an error"
        );

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(None) => {}
            other => panic!("unexpected poll outcome: {:?}", other),
        }
    }

    #[tokio::test]
    async fn first_body_trailers_are_propagated() {
        let trailers = {
            let trls = vec![(
                HeaderName::from_static("fourty-two"),
                HeaderValue::from_static("42"),
            )]
            .into_iter()
            .collect::<HeaderMap>();
            ready(Some(Ok(trls)))
        };

        let mut body = {
            type Data = Bytes;
            let first = Full::<Data>::new("hello ".into()).with_trailers(trailers);
            let second = Full::<Data>::new("world!".into());
            first.chain(second)
        };

        let waker = futures_util::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(Some(Ok(frame))) => {
                let frame = frame.into_data().expect("should yield data");
                assert_eq!(frame, "hello ");
            }
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(Some(Ok(frame))) => {
                let frame = frame.into_data().expect("should yield data");
                assert_eq!(frame, "world!");
            }
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(Some(Ok(frame))) => {
                let trailers = frame.into_trailers().expect("should yield trailers");
                let value = trailers
                    .get("fourty-two")
                    .map(HeaderValue::to_str)
                    .transpose()
                    .expect("header is a string")
                    .expect("header exists");
                assert_eq!(value, "42");
            }
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        assert!(
            body.is_end_stream(),
            "body is finished after returning trailers"
        );

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(None) => {}
            other => panic!("unexpected poll outcome: {:?}", other),
        }
    }

    #[tokio::test]
    async fn second_body_trailers_are_propagated() {
        let trailers = {
            let trls = vec![(
                HeaderName::from_static("fourty-two"),
                HeaderValue::from_static("42"),
            )]
            .into_iter()
            .collect::<HeaderMap>();
            ready(Some(Ok(trls)))
        };

        let mut body = {
            type Data = Bytes;
            let first = Full::<Data>::new("hello ".into());
            let second = Full::<Data>::new("world!".into()).with_trailers(trailers);
            first.chain(second)
        };

        let waker = futures_util::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(Some(Ok(frame))) => {
                let frame = frame.into_data().expect("should yield data");
                assert_eq!(frame, "hello ");
            }
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(Some(Ok(frame))) => {
                let frame = frame.into_data().expect("should yield data");
                assert_eq!(frame, "world!");
            }
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(Some(Ok(frame))) => {
                let trailers = frame.into_trailers().expect("should yield trailers");
                let value = trailers
                    .get("fourty-two")
                    .map(HeaderValue::to_str)
                    .transpose()
                    .expect("header is a string")
                    .expect("header exists");
                assert_eq!(value, "42");
            }
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        assert!(
            body.is_end_stream(),
            "body is finished after returning trailers"
        );

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(None) => {}
            other => panic!("unexpected poll outcome: {:?}", other),
        }
    }

    #[tokio::test]
    async fn body_trailers_are_consolidated() {
        type Data = Bytes;

        let first = {
            let data = Full::<Data>::new("hello ".into());
            let trls = vec![
                (
                    HeaderName::from_static("fourty-two"),
                    HeaderValue::from_static("42"),
                ),
                (
                    HeaderName::from_static("both"),
                    HeaderValue::from_static("alpha"),
                ),
            ]
            .into_iter()
            .collect::<HeaderMap>();
            data.with_trailers(ready(Some(Ok(trls))))
        };

        let second = {
            let data = Full::<Data>::new("world!".into());
            let trls = vec![
                (
                    HeaderName::from_static("ten"),
                    HeaderValue::from_static("10"),
                ),
                (
                    HeaderName::from_static("both"),
                    HeaderValue::from_static("beta"),
                ),
            ]
            .into_iter()
            .collect::<HeaderMap>();
            data.with_trailers(ready(Some(Ok(trls))))
        };

        let mut body = first.chain(second);

        let waker = futures_util::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(Some(Ok(frame))) => {
                let frame = frame.into_data().expect("should yield data");
                assert_eq!(frame, "hello ");
            }
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(Some(Ok(frame))) => {
                let frame = frame.into_data().expect("should yield data");
                assert_eq!(frame, "world!");
            }
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(Some(Ok(frame))) => {
                let trailers = frame.into_trailers().expect("should yield trailers");
                assert_eq!(trailers.len(), 3);

                let value = trailers
                    .get("fourty-two")
                    .map(HeaderValue::to_str)
                    .transpose()
                    .expect("header is a string")
                    .expect("header exists");
                assert_eq!(value, "42");

                let value = trailers
                    .get("ten")
                    .map(HeaderValue::to_str)
                    .transpose()
                    .expect("header is a string")
                    .expect("header exists");
                assert_eq!(value, "10");

                // In the event of conflicts, the second body's headers take precendent.
                let both = trailers
                    .get_all("both")
                    .iter()
                    .map(|s| s.to_str().unwrap().to_string())
                    .collect::<Vec<String>>();
                assert_eq!(both.len(), 1);
                assert_eq!(both, ["beta"]);
            }
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(None) => {}
            other => panic!("unexpected poll outcome: {:?}", other),
        }
    }
}
