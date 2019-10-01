//! Next futures for `Body`

use crate::Body;

use core::future::Future;
use core::pin::Pin;
use core::task;

#[derive(Debug)]
/// Future that resolves to the next data chunk from `Body`
pub struct Next<'a, T: ?Sized>(pub(crate) &'a mut T);

impl<'a, T: Body + Unpin + ?Sized> Future for Next<'a, T> {
    type Output = Option<Result<T::Data, T::Error>>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        Pin::new(&mut self.0).poll_data(ctx)
    }
}

#[derive(Debug)]
/// Future that resolves to the optional trailers from `Body`
pub struct Trailers<'a, T: ?Sized>(pub(crate) &'a mut T);

impl<'a, T: Body + Unpin + ?Sized> Future for Trailers<'a, T> {
    type Output = Result<Option<http::HeaderMap>, T::Error>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        Pin::new(&mut self.0).poll_trailers(ctx)
    }
}
