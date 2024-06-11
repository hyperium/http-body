//! A body backed by a channel.

use std::{
    fmt::Display,
    pin::Pin,
    task::{Context, Poll},
};

use bytes::Buf;
use http::HeaderMap;
use http_body::{Body, Frame};
use tokio::sync::mpsc;

/// A body backed by a channel.
pub struct Channel<D, E = std::convert::Infallible> {
    rx_frame: mpsc::Receiver<Frame<D>>,
    rx_error: mpsc::Receiver<E>,
}

impl<D, E> Channel<D, E> {
    /// Create a new channel body.
    ///
    /// The channel will buffer up to the provided number of messages. Once the buffer is full,
    /// attempts to send new messages will wait until a message is received from the channel. The
    /// provided buffer capacity must be at least 1.
    pub fn new(buffer: usize) -> (Sender<D, E>, Self) {
        let (tx_frame, rx_frame) = mpsc::channel(buffer);
        let (tx_error, rx_error) = mpsc::channel(1);
        (Sender { tx_frame, tx_error }, Self { rx_frame, rx_error })
    }
}

impl<D, E> Body for Channel<D, E>
where
    D: Buf,
{
    type Data = D;
    type Error = E;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match self.rx_frame.poll_recv(cx) {
            Poll::Ready(frame) => return Poll::Ready(frame.map(Ok)),
            Poll::Pending => {}
        }

        match self.rx_error.poll_recv(cx) {
            Poll::Ready(err) => return Poll::Ready(err.map(Err)),
            Poll::Pending => {}
        }

        Poll::Pending
    }
}

impl<D, E> std::fmt::Debug for Channel<D, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Channel")
            .field("rx_frame", &self.rx_frame)
            .field("rx_error", &self.rx_error)
            .finish()
    }
}

/// A sender half created through [`Channel::new`].
pub struct Sender<D, E = std::convert::Infallible> {
    tx_frame: mpsc::Sender<Frame<D>>,
    tx_error: mpsc::Sender<E>,
}

impl<D, E> Sender<D, E> {
    /// Send a frame on the channel.
    pub async fn send(&self, frame: Frame<D>) -> Result<(), SendError> {
        self.tx_frame.send(frame).await.map_err(|_| SendError)
    }

    /// Send data on data channel.
    pub async fn send_data(&self, buf: D) -> Result<(), SendError> {
        self.send(Frame::data(buf)).await
    }

    /// Send trailers on trailers channel.
    pub async fn send_trailers(&self, trailers: HeaderMap) -> Result<(), SendError> {
        self.send(Frame::trailers(trailers)).await
    }

    /// Aborts the body in an abnormal fashion.
    pub fn abort(self, error: E) {
        match self.tx_error.try_send(error) {
            Ok(_) => {}
            Err(err) => {
                match err {
                    mpsc::error::TrySendError::Full(_) => {
                        // Channel::new creates the error channel with space for 1 message and we
                        // only send once because this method consumes `self`. So the receiver
                        // can't be full.
                        unreachable!("error receiver should never be full")
                    }
                    mpsc::error::TrySendError::Closed(_) => {}
                }
            }
        }
    }
}

impl<D, E> std::fmt::Debug for Sender<D, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sender")
            .field("tx_frame", &self.tx_frame)
            .field("tx_error", &self.tx_error)
            .finish()
    }
}

/// The error returned if [`Sender`] fails to send because the receiver is closed.
#[derive(Debug)]
#[non_exhaustive]
pub struct SendError;

impl Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "failed to send frame")
    }
}

impl std::error::Error for SendError {}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use http::{HeaderName, HeaderValue};

    use crate::BodyExt;

    use super::*;

    #[tokio::test]
    async fn works() {
        let (tx, body) = Channel::<Bytes>::new(1024);

        tokio::spawn(async move {
            tx.send_data(Bytes::from("Hel")).await.unwrap();
            tx.send_data(Bytes::from("lo!")).await.unwrap();

            let mut trailers = HeaderMap::new();
            trailers.insert(
                HeaderName::from_static("foo"),
                HeaderValue::from_static("bar"),
            );
            tx.send_trailers(trailers).await.unwrap();
        });

        let collected = body.collect().await.unwrap();
        assert_eq!(collected.trailers().unwrap()["foo"], "bar");
        assert_eq!(collected.to_bytes(), "Hello!");
    }
}
