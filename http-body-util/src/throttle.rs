use bytes::Buf;
use http::HeaderMap;
use http_body::{Body, SizeHint};
use pin_project_lite::pin_project;
use std::{
    convert::{TryFrom, TryInto},
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::time::{sleep, Instant, Sleep};

#[derive(Debug)]
enum State {
    Waiting(Pin<Box<Sleep>>, Instant),
    Ready(Instant),
    Init,
}

pin_project! {
    /// A throttled body.
    #[derive(Debug)]
    #[cfg_attr(docsrs, doc(cfg(feature = "tokio")))]
    pub struct Throttle<B> {
        #[pin]
        inner: B,
        state: State,
        cursor: f64,
        byte_rate: f64,
    }
}

impl<B> Throttle<B> {
    /// Create a new `Throttle`.
    ///
    /// # Panic
    ///
    /// Will panic if milliseconds in `duration` is larger than `u32::MAX`.
    pub fn new(body: B, duration: Duration, bytes: u32) -> Self {
        let bytes = f64::from(bytes);
        let duration = f64::from(u32::try_from(duration.as_millis()).expect("duration too large"));

        let byte_rate = bytes / duration;

        Self {
            inner: body,
            state: State::Init,
            cursor: 0.0,
            byte_rate,
        }
    }
}

impl<B: Body> Body for Throttle<B> {
    type Data = B::Data;
    type Error = B::Error;

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        let mut this = self.project();

        loop {
            match this.state {
                State::Waiting(sleep, time) => match sleep.as_mut().poll(cx) {
                    Poll::Ready(()) => {
                        let byte_rate = *this.byte_rate;
                        let mut elapsed = to_f64(time.elapsed().as_millis());

                        if elapsed > 2000.0 {
                            elapsed = 2000.0;
                        }

                        *this.cursor += elapsed * byte_rate;
                        *this.state = State::Ready(Instant::now());
                    }
                    Poll::Pending => return Poll::Pending,
                },
                State::Ready(time) => match this.inner.as_mut().poll_data(cx) {
                    Poll::Ready(Some(Ok(data))) => {
                        let byte_count = to_f64(data.remaining());
                        let byte_rate = *this.byte_rate;

                        *this.cursor -= byte_count;

                        if *this.cursor <= 0.0 {
                            let wait_millis = this.cursor.abs() / byte_rate;
                            let duration = Duration::from_millis(wait_millis as u64);

                            *this.state = State::Waiting(Box::pin(sleep(duration)), *time);
                        }

                        return Poll::Ready(Some(Ok(data)));
                    }
                    poll_result => return poll_result,
                },
                State::Init => *this.state = State::Ready(Instant::now()),
            }
        }
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        self.project().inner.poll_trailers(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

fn to_f64(n: impl TryInto<u32>) -> f64 {
    f64::from(n.try_into().unwrap_or(u32::MAX))
}

#[cfg(test)]
mod tests {
    use crate::{StreamBody, Throttle};
    use bytes::Bytes;
    use http_body::Body;
    use std::{convert::Infallible, time::Duration};
    use tokio::time::Instant;

    #[tokio::test(start_paused = true)]
    async fn per_second_256() {
        let start = Instant::now();

        let chunks: Vec<Result<Bytes, Infallible>> = vec![
            Ok(Bytes::from(vec![0u8; 128])),
            Ok(Bytes::from(vec![0u8; 128])),
            Ok(Bytes::from(vec![0u8; 256])),
            Ok(Bytes::from(vec![0u8; 128])),
            Ok(Bytes::from(vec![0u8; 128])),
        ];
        let stream = futures_util::stream::iter(chunks);
        let mut body = Throttle::new(StreamBody::new(stream), Duration::from_secs(1), 256);

        assert_eq!(body.data().await.unwrap().unwrap().as_ref(), [0u8; 128]);
        assert!(start.elapsed().is_zero()); // Throttling starts after first chunk.

        assert_eq!(body.data().await.unwrap().unwrap().as_ref(), [0u8; 128]);
        assert_eq!(start.elapsed(), Duration::from_millis(500));

        assert_eq!(body.data().await.unwrap().unwrap().as_ref(), [0u8; 256]);
        assert_eq!(start.elapsed(), Duration::from_millis(1000));

        assert_eq!(body.data().await.unwrap().unwrap().as_ref(), [0u8; 128]);
        assert_eq!(start.elapsed(), Duration::from_millis(2000));

        assert_eq!(body.data().await.unwrap().unwrap().as_ref(), [0u8; 128]);
        assert_eq!(start.elapsed(), Duration::from_millis(2500));

        assert!(body.data().await.is_none());
    }
}
