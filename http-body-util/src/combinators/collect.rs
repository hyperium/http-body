use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::Future;
use http_body::Body;
use pin_project_lite::pin_project;

pin_project! {
    /// Future that resolves into a `Collected`.
    pub struct Collect<T: ?Sized> {
        #[pin]
        pub(crate) body: T
    }
}

impl<T: Body + ?Sized> Future for Collect<T> {
    type Output = Result<crate::Collected<T::Data>, T::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> std::task::Poll<Self::Output> {
        let mut collected = crate::Collected::default();

        let mut me = self.project();

        loop {
            let frame = futures_util::ready!(me.body.as_mut().poll_frame(cx));

            let frame = if let Some(frame) = frame {
                frame?
            } else {
                return Poll::Ready(Ok(collected));
            };

            collected.push_frame(frame);
        }
    }
}
