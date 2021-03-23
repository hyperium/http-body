//! Adapters for turning various types into [`Body`]s.
//!
//! [`Body`]: crate::Body

mod async_read;

pub use async_read::AsyncReadBody;
