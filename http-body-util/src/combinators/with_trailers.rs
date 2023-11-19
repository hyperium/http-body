use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::ready;
use http::HeaderMap;
use http_body::{Body, Frame};
use pin_project_lite::pin_project;

pin_project! {
    /// Adds trailers to a body.
    ///
    /// See [`BodyExt::with_trailers`] for more details.
    pub struct WithTrailers<T, F> {
        #[pin]
        state: State<T, F>,
    }
}

impl<T, F> WithTrailers<T, F> {
    pub(crate) fn new(body: T, trailers: F) -> Self {
        Self {
            state: State::PollBody {
                body,
                trailers: Some(trailers),
            },
        }
    }
}

pin_project! {
    #[project = StateProj]
    enum State<T, F> {
        PollBody {
            #[pin]
            body: T,
            trailers: Option<F>,
        },
        PollTrailers {
            #[pin]
            trailers: F,
        },
        Trailers {
            trailers: Option<HeaderMap>,
        }
    }
}

impl<T, F> Body for WithTrailers<T, F>
where
    T: Body,
    F: Future<Output = Option<Result<HeaderMap, T::Error>>>,
{
    type Data = T::Data;
    type Error = T::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        loop {
            let mut this = self.as_mut().project();

            let new_state: State<_, _> = match this.state.as_mut().project() {
                StateProj::PollBody { body, trailers } => match ready!(body.poll_frame(cx)?) {
                    Some(frame) => {
                        return Poll::Ready(Some(Ok(frame)));
                    }
                    None => {
                        let trailers = trailers.take().unwrap();
                        State::PollTrailers { trailers }
                    }
                },
                StateProj::PollTrailers { trailers } => {
                    let trailers = ready!(trailers.poll(cx)?);
                    State::Trailers { trailers }
                }
                StateProj::Trailers { trailers } => {
                    return Poll::Ready(trailers.take().map(Frame::trailers).map(Ok));
                }
            };

            this.state.set(new_state);
        }
    }

    #[inline]
    fn is_end_stream(&self) -> bool {
        match &self.state {
            State::PollBody { body, .. } => body.is_end_stream(),
            State::PollTrailers { .. } | State::Trailers { .. } => true,
        }
    }

    #[inline]
    fn size_hint(&self) -> http_body::SizeHint {
        match &self.state {
            State::PollBody { body, .. } => body.size_hint(),
            State::PollTrailers { .. } | State::Trailers { .. } => Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use bytes::Bytes;
    use http::{HeaderMap, HeaderName, HeaderValue};

    use crate::{BodyExt, Full};

    #[allow(unused_imports)]
    use super::*;

    #[tokio::test]
    async fn works() {
        let mut trailers = HeaderMap::new();
        trailers.insert(
            HeaderName::from_static("foo"),
            HeaderValue::from_static("bar"),
        );

        let body =
            Full::<Bytes>::from("hello").with_trailers(std::future::ready(Some(
                Ok::<_, Infallible>(trailers.clone()),
            )));

        futures_util::pin_mut!(body);
        let waker = futures_util::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        let data = unwrap_ready(body.as_mut().poll_frame(&mut cx))
            .unwrap()
            .unwrap()
            .into_data()
            .unwrap();
        assert_eq!(data, "hello");

        let body_trailers = unwrap_ready(body.as_mut().poll_frame(&mut cx))
            .unwrap()
            .unwrap()
            .into_trailers()
            .unwrap();
        assert_eq!(body_trailers, trailers);

        assert!(unwrap_ready(body.as_mut().poll_frame(&mut cx)).is_none());
    }

    fn unwrap_ready<T>(poll: Poll<T>) -> T {
        match poll {
            Poll::Ready(t) => t,
            Poll::Pending => panic!("pending"),
        }
    }
}
