extern crate bytes;
extern crate futures;
extern crate http;
extern crate tokio_buf;

use bytes::Buf;
use futures::{Async, Poll};
use http::HeaderMap;
use tokio_buf::{BufStream, SizeHint};

/// Trait representing a streaming body of a Request or Response.
pub trait Body {
    /// Values yielded by the `Body`.
    type Data: Buf;

    /// The error type this `BufStream` might generate.
    type Error;

    /// Attempt to pull out the next data buffer of this stream.
    fn poll_data(&mut self) -> Poll<Option<Self::Data>, Self::Error>;

    /// Returns the bounds on the remaining length of the stream.
    fn size_hint(&self) -> SizeHint {
        SizeHint::default()
    }

    /// Poll for an optional **single** `HeaderMap` of trailers.
    fn poll_trailers(&mut self) -> Poll<Option<HeaderMap>, Self::Error>;

    /// Returns `true` when the end of stream has been reached.
    ///
    /// An end of stream means that both `poll_data` and `poll_trailers` will
    /// return `None`.
    ///
    /// A return value of `false` **does not** guarantee that a value will be
    /// returned from `poll_stream` or `poll_trailers`.
    fn is_end_stream(&self) -> bool {
        false
    }
}

impl<T: BufStream> Body for T {
    type Data = T::Item;
    type Error = T::Error;

    fn poll_data(&mut self) -> Poll<Option<Self::Data>, Self::Error> {
        BufStream::poll_buf(self)
    }

    fn size_hint(&self) -> SizeHint {
        BufStream::size_hint(self)
    }

    fn poll_trailers(&mut self) -> Poll<Option<HeaderMap>, Self::Error> {
        Ok(Async::Ready(None))
    }

    fn is_end_stream(&self) -> bool {
        let size_hint = self.size_hint();

        size_hint
            .upper()
            .map(|upper| upper == 0 && upper == size_hint.lower())
            .unwrap_or(false)
    }
}
