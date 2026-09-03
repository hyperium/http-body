use bytes::Buf;
use http_body::{Body, Frame, SizeHint};
use std::{
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

/// A [`Body`] that always returns [`Poll::Pending`] when polled.
///
/// A [`Pending<D, E>`] is a body that will continue to yield [`Poll::Pending`] when
/// [`Body::poll_frame()`] is called. The `D` and `E` generics are used to specify [`Body::Data`]
/// and [`Body::Error`].
///
/// This is like [`std::future::Pending`], but for response bodies. It represents a
/// body that never resolves, which is often useful for writing test coverage.
#[derive(Debug, Default)]
pub struct Pending<D, E> {
    data: PhantomData<D>,
    error: PhantomData<E>,
}

// === impl Pending ===

impl<D, E> Pending<D, E> {
    /// Returns a new [`Pending`] that yields [`Poll::Pending`] when polled.
    ///
    /// # Examples
    ///
    /// ```
    /// # use bytes::Bytes;
    /// # use http_body::Body;
    /// # use http_body_util::Pending;
    /// # use std::pin::Pin;
    /// # use std::task::{Context, Poll};
    /// #
    /// type Error = Box<dyn std::error::Error + Send + 'static>;
    ///
    /// let mut body = Pending::<Bytes, Error>::new();
    ///
    /// let waker = futures_util::task::noop_waker();
    /// let mut cx = Context::from_waker(&waker);
    ///
    /// // The body yields `Pending` when polled.
    /// match Pin::new(&mut body).poll_frame(&mut cx) {
    ///     Poll::Pending => {}
    ///     other => panic!(),
    /// }
    /// ```
    pub fn new() -> Self {
        Self {
            data: PhantomData,
            error: PhantomData,
        }
    }
}

impl<D, E> Body for Pending<D, E>
where
    E: Unpin,
    D: Buf + Unpin,
{
    type Data = D;
    type Error = E;

    fn poll_frame(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Poll::Pending
    }

    fn is_end_stream(&self) -> bool {
        false
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::default()
    }
}

#[cfg(test)]
mod pending_body_tests {
    use super::Pending;
    use bytes::Bytes;
    use http_body::Body;
    use std::{
        ops::Not,
        pin::Pin,
        task::{Context, Poll},
    };

    #[test]
    fn yields_poll_pending() {
        type Error = &'static str;

        let mut body = Pending::<Bytes, Error>::new();

        assert!(
            body.is_end_stream().not(),
            "body is not finished until polled"
        );
        assert_eq!(body.size_hint().lower(), 0);
        assert_eq!(body.size_hint().upper(), None);

        let waker = futures_util::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Pending => {}
            other => panic!("unexpected poll outcome: {:?}", other),
        }

        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Pending => {}
            other => panic!("unexpected poll outcome: {:?}", other),
        }
    }
}
