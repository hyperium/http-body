#![deny(
    missing_debug_implementations,
    missing_docs,
    unreachable_pub,
    rustdoc::broken_intra_doc_links
)]
#![cfg_attr(docsrs, feature(doc_auto_cfg, doc_cfg))]
#![cfg_attr(test, deny(warnings))]

//! Utilities for [`http_body::Body`].
//!
//! [`BodyExt`] adds extensions to the common trait.
//!
//! [`Empty`] and [`Full`] provide simple implementations.

pub mod combinators;
mod empty;
mod full;
mod limited;
mod stream;

#[cfg(feature = "tokio")]
mod throttle;

use self::combinators::{BoxBody, MapData, MapErr, UnsyncBoxBody};
pub use self::empty::Empty;
pub use self::full::Full;
pub use self::limited::{LengthLimitError, Limited};
pub use self::stream::StreamBody;

#[cfg(feature = "tokio")]
pub use self::throttle::Throttle;

/// An extension trait for [`http_body::Body`] adding various combinators and adapters
pub trait BodyExt: http_body::Body {
    /// Maps this body's data value to a different value.
    fn map_data<F, B>(self, f: F) -> MapData<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Data) -> B,
        B: bytes::Buf,
    {
        MapData::new(self, f)
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
}

impl<T: ?Sized> BodyExt for T where T: http_body::Body {}
