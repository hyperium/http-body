//! Combinators for the `Body` trait.

mod box_body;
mod into_stream;
mod map_data;
mod map_err;

pub use self::{box_body::BoxBody, into_stream::IntoStream, map_data::MapData, map_err::MapErr};
