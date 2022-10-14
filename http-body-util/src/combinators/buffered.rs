use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::Future;
use http_body::Body;
use pin_project_lite::pin_project;

pin_project! {
    /// Future that resolves into a `Buffered`.
    pub struct Buffered<T: ?Sized> {
        #[pin]
        pub(crate) body: T
    }
}

impl<T: Body + Unpin + ?Sized> Future for Buffered<T> {
    type Output = Result<crate::Buffered<T::Data>, T::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> std::task::Poll<Self::Output> {
        let mut buffered = crate::Buffered::default();

        let mut me = self.project();

        loop {
            let frame = futures_util::ready!(Pin::new(&mut me.body).poll_frame(cx));

            let frame = if let Some(frame) = frame {
                frame?
            } else {
                return Poll::Ready(Ok(buffered));
            };

            buffered.push_frame(frame);
        }
    }
}
