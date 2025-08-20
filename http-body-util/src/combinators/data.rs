use http_body::Body;

use core::future::Future;
use core::pin::Pin;
use core::task;

#[must_use = "futures don't do anything unless polled"]
#[derive(Debug)]
/// Future that resolves to the next data chunk from `Body`
pub struct Data<'a, T: ?Sized>(pub(crate) &'a mut T);

impl<'a, T: Body + Unpin + ?Sized> Future for Data<'a, T> {
    type Output = Option<Result<T::Data, T::Error>>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        match Pin::new(&mut self.0).poll_frame(ctx) {
            task::Poll::Ready(Some(Ok(frame))) => {
                if let Ok(data) = frame.into_data() {
                    task::Poll::Ready(Some(Ok(data)))
                } else {
                    task::Poll::Pending
                }
            },
            task::Poll::Ready(Some(Err(e))) => task::Poll::Ready(Some(Err(e))),
            task::Poll::Ready(None) => task::Poll::Ready(None),
            task::Poll::Pending => task::Poll::Pending,
        }
    }
}
