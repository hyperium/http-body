#![deny(
    missing_debug_implementations,
    missing_docs,
    unreachable_pub,
    rustdoc::broken_intra_doc_links
)]
#![cfg_attr(test, deny(warnings))]

//! Utilities for [`http_body::Body`].
//!
//! [`BodyExt`] adds extensions to the common trait.
//!
//! [`Empty`] and [`Full`] provide simple implementations.

mod collected;
pub mod combinators;
mod either;
mod empty;
mod full;
mod limited;
mod stream;

mod util;

use self::combinators::{BoxBody, MapErr, MapFrame, UnsyncBoxBody};

pub use self::collected::Collected;
pub use self::either::Either;
pub use self::empty::Empty;
pub use self::full::Full;
pub use self::limited::{LengthLimitError, Limited};
pub use self::stream::{BodyStream, StreamBody};

/// An extension trait for [`http_body::Body`] adding various combinators and adapters
pub trait BodyExt: http_body::Body {
    /// Returns a future that resolves to the next [`Frame`], if any.
    ///
    /// [`Frame`]: combinators::Frame
    fn frame(&mut self) -> combinators::Frame<'_, Self>
    where
        Self: Unpin,
    {
        combinators::Frame(self)
    }

    /// Maps this body's frame to a different kind.
    fn map_frame<F, B>(self, f: F) -> MapFrame<Self, F>
    where
        Self: Sized,
        F: FnMut(http_body::Frame<Self::Data>) -> http_body::Frame<B>,
        B: bytes::Buf,
    {
        MapFrame::new(self, f)
    }

    /// Maps this body's error value to a different value.
    fn map_err<F, E>(self, f: F) -> MapErr<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Error) -> E,
    {
        MapErr::new(self, f)
    }

    /// Turn this body into a boxed trait object.
    fn boxed(self) -> BoxBody<Self::Data, Self::Error>
    where
        Self: Sized + Send + Sync + 'static,
    {
        BoxBody::new(self)
    }

    /// Turn this body into a boxed trait object that is !Sync.
    fn boxed_unsync(self) -> UnsyncBoxBody<Self::Data, Self::Error>
    where
        Self: Sized + Send + 'static,
    {
        UnsyncBoxBody::new(self)
    }

    /// Turn this body into [`Collected`] body which will collect all the DATA frames
    /// and trailers.
    fn collect(self) -> combinators::Collect<Self>
    where
        Self: Sized,
    {
        combinators::Collect {
            body: self,
            collected: Some(crate::Collected::default()),
        }
    }
}

impl<T: ?Sized> BodyExt for T where T: http_body::Body {}
