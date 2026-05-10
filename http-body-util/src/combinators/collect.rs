use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::ready;
use http_body::Body;
use pin_project_lite::pin_project;

pin_project! {
    /// Future that resolves into a [`Collected`].
    ///
    /// [`Collected`]: crate::Collected
    pub struct Collect<T>
    where
        T: Body,
        T: ?Sized,
    {
        pub(crate) collected: Option<crate::Collected<T::Data>>,
        #[pin]
        pub(crate) body: T,
    }
}

impl<T: Body + ?Sized> Future for Collect<T> {
    type Output = Result<crate::Collected<T::Data>, T::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> std::task::Poll<Self::Output> {
        let mut me = self.project();
        // Bound each poll so collecting an always-ready body remains fair to
        // cooperative async runtimes.
        let mut budget = 128usize;

        loop {
            if budget == 0 {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
            budget -= 1;

            let frame = ready!(me.body.as_mut().poll_frame(cx));

            let frame = if let Some(frame) = frame {
                frame?
            } else {
                return Poll::Ready(Ok(me.collected.take().expect("polled after complete")));
            };

            me.collected.as_mut().unwrap().push_frame(frame);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        convert::Infallible,
        future::Future,
        pin::Pin,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        task::{Context, Poll, Wake, Waker},
    };

    use bytes::Bytes;
    use http_body::{Body, Frame, SizeHint};

    use crate::BodyExt as _;

    struct ReadyFrames {
        remaining: usize,
    }

    impl ReadyFrames {
        fn new(remaining: usize) -> Self {
            Self { remaining }
        }
    }

    impl Body for ReadyFrames {
        type Data = Bytes;
        type Error = Infallible;

        fn poll_frame(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
            if self.remaining == 0 {
                return Poll::Ready(None);
            }

            self.remaining -= 1;
            Poll::Ready(Some(Ok(Frame::data(Bytes::from_static(b"x")))))
        }

        fn size_hint(&self) -> SizeHint {
            SizeHint::with_exact(self.remaining as u64)
        }
    }

    #[derive(Default)]
    struct CountWake {
        wakes: AtomicUsize,
    }

    impl CountWake {
        fn count(&self) -> usize {
            self.wakes.load(Ordering::Relaxed)
        }
    }

    impl Wake for CountWake {
        fn wake(self: Arc<Self>) {
            self.wakes.fetch_add(1, Ordering::Relaxed);
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.wakes.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn collect_yields_after_draining_ready_frame_budget() {
        let mut collect = Box::pin(ReadyFrames::new(129).collect());
        let wake = Arc::new(CountWake::default());
        let waker = Waker::from(Arc::clone(&wake));
        let mut cx = Context::from_waker(&waker);

        assert!(matches!(collect.as_mut().poll(&mut cx), Poll::Pending));
        assert_eq!(wake.count(), 1);

        let collected = match collect.as_mut().poll(&mut cx) {
            Poll::Ready(Ok(collected)) => collected,
            Poll::Ready(Err(error)) => match error {},
            Poll::Pending => panic!("collect should complete on the second poll"),
        };

        assert_eq!(collected.to_bytes().len(), 129);
    }
}
