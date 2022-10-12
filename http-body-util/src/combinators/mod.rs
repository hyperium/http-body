//! Combinators for the `Body` trait.

mod box_body;
mod frame;
mod map_err;
mod map_frame;

pub use self::{
    box_body::{BoxBody, UnsyncBoxBody},
    frame::Frame,
    map_err::MapErr,
    map_frame::MapFrame,
};
