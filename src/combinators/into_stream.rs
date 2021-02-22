use crate::Body;
use futures_core::stream::Stream;
use pin_project_lite::pin_project;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

pin_project! {
    /// Stream returned by the [`into_stream`] combinator.
    ///
    /// [`into_stream`]: crate::util::BodyExt::into_stream
    #[derive(Debug, Clone, Copy)]
    pub struct IntoStream<B> {
        #[pin]
        body: B
    }
}

impl<B> IntoStream<B> {
    pub(crate) fn new(body: B) -> Self {
        Self { body }
    }

    /// Get a reference to the inner body
    pub fn get_ref(&self) -> &B {
        &self.body
    }

    /// Get a mutable reference to the inner body
    pub fn get_mut(&mut self) -> &mut B {
        &mut self.body
    }

    /// Get a pinned mutable reference to the inner body
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut B> {
        self.project().body
    }

    /// Consume `self`, returning the inner body
    pub fn into_inner(self) -> B {
        self.body
    }
}

impl<B> Stream for IntoStream<B>
where
    B: Body,
{
    type Item = Result<B::Data, B::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().body.poll_data(cx)
    }
}
