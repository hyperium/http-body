//! Combinators for the `Body` trait.

mod box_body;
mod buffered;
mod frame;
mod map_err;
mod map_frame;

pub use self::{
    box_body::{BoxBody, UnsyncBoxBody},
    buffered::Buffered,
    frame::Frame,
    map_err::MapErr,
    map_frame::MapFrame,
};
