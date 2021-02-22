//! An extension trait for `Body`s that provides a variety of convenient adapters

mod box_body;
mod into_stream;
mod map_data;
mod map_err;

pub(crate) use self::{
    box_body::BoxBody, into_stream::IntoStream, map_data::MapData, map_err::MapErr,
};
