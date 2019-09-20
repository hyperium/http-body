//! Next futures for `Body`

use crate::Body;

use core::future::Future;
use core::pin::Pin;
use core::task;

#[derive(Debug)]
/// Future that resolves to the next data chunk from `Body`
pub struct NextData<'a, T>(pub(crate) &'a mut T);

impl<'a, T: Body + Unpin> Future for NextData<'a, T> {
    type Output = Option<Result<T::Data, T::Error>>;

    fn poll(self: Pin<&mut Self>, ctx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        let body = unsafe { self.map_unchecked_mut(|this| &mut this.0) };

        Body::poll_data(body, ctx)
    }
}

#[derive(Debug)]
/// Future that resolves to the optional trailers from `Body`
pub struct NextTrailers<'a, T>(pub(crate) &'a mut T);

impl<'a, T: Body + Unpin> Future for NextTrailers<'a, T> {
    type Output = Result<Option<http::HeaderMap>, T::Error>;

    fn poll(self: Pin<&mut Self>, ctx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        let body = unsafe { self.map_unchecked_mut(|this| &mut this.0) };

        Body::poll_trailers(body, ctx)
    }
}
