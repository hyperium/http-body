//! An extension trait for `Body`s that provides a variety of convenient adapters

use crate::Body;
use bytes::Buf;

mod box_body;
mod into_stream;
mod map_data;
mod map_err;

pub use self::{box_body::BoxBody, into_stream::IntoStream, map_data::MapData, map_err::MapErr};

/// An extension trait for [`Body`]s that provides a variety of convenient adapters
///
/// [`Body`]: Body
pub trait BodyExt: Body {
    /// Maps this body's data value to a different value.
    fn map_data<F, B>(self, f: F) -> MapData<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Data) -> B,
        B: Buf,
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

    /// Turn this body into a [`Stream`].
    ///
    /// [`Stream`]: https://docs.rs/futures/latest/futures/stream/trait.Stream.html
    fn into_stream(self) -> IntoStream<Self>
    where
        Self: Sized,
    {
        IntoStream::new(self)
    }
}

impl<T> BodyExt for T where T: Body {}
