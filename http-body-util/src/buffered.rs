use bytes::{Buf, Bytes};
use http::HeaderMap;
use http_body::Frame;

use crate::util::BufList;

/// A collected body produced by [`BodyExt::collect`] which collects all the DATA frames
/// and trailers.
#[derive(Debug)]
pub struct Collected<B> {
    pub(crate) bufs: BufList<B>,
    pub(crate) trailers: Option<HeaderMap>,
}

impl<B: Buf> Collected<B> {
    /// If there is a trailers frame buffered, returns a reference to it.
    ///
    /// Returns `None` if the body contained no trailers.
    pub fn trailers(&self) -> Option<&HeaderMap> {
        self.trailers.as_ref()
    }

    /// Aggregate this buffered into a [`Buf`].
    pub fn aggregate(self) -> impl Buf {
        self.bufs
    }

    /// Convert this body into a [`Bytes`].
    pub fn to_bytes(mut self) -> Bytes {
        self.bufs.copy_to_bytes(self.bufs.remaining())
    }

    pub(crate) fn push_frame(&mut self, frame: Frame<B>) {
        if frame.is_data() {
            let data = frame.into_data().unwrap();
            self.bufs.push(data);
        } else if frame.is_trailers() {
            let trailers = frame.into_trailers().unwrap();

            if let Some(current) = &mut self.trailers {
                current.extend(trailers.into_iter());
            } else {
                self.trailers = Some(trailers);
            }
        }
    }
}

impl<B> Default for Collected<B> {
    fn default() -> Self {
        Self {
            bufs: BufList::default(),
            trailers: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::{Infallible, TryInto};

    use futures_util::stream;

    use crate::{BodyExt, Full, StreamBody};

    use super::*;

    #[tokio::test]
    async fn full_body() {
        let body = Full::new(&b"hello"[..]);

        let buffered = body.collect().await.unwrap();

        let mut buf = buffered.to_bytes();

        assert_eq!(&buf.copy_to_bytes(buf.remaining())[..], &b"hello"[..]);
    }

    #[tokio::test]
    async fn segmented_body() {
        let bufs = [&b"hello"[..], &b"world"[..], &b"!"[..]];

        let body = StreamBody::new(stream::iter(bufs.map(Frame::data).map(Ok::<_, Infallible>)));

        let buffered = body.collect().await.unwrap();

        let mut buf = buffered.to_bytes();

        assert_eq!(&buf.copy_to_bytes(buf.remaining())[..], b"helloworld!");
    }

    #[tokio::test]
    async fn trailers() {
        let mut trailers = HeaderMap::new();
        trailers.insert("this", "a trailer".try_into().unwrap());
        let bufs = [
            Frame::data(&b"hello"[..]),
            Frame::data(&b"world"[..]),
            Frame::trailers(trailers.clone()),
        ];

        let body = StreamBody::new(stream::iter(bufs.map(Ok::<_, Infallible>)));

        let buffered = body.collect().await.unwrap();

        assert_eq!(&trailers, buffered.trailers().unwrap());

        let mut buf = buffered.to_bytes();

        assert_eq!(&buf.copy_to_bytes(buf.remaining())[..], b"helloworld");
    }
}
