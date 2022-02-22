use bytes::Buf;

use crate::buf::BufList;
use crate::Body;

/// Aggregate the data buffers from a body asynchronously.
///
/// The returned `impl Buf` groups the `Buf`s from the `HttpBody` without
/// copying them. This is ideal if you don't require a contiguous buffer.
///
/// # Note
///
/// Care needs to be taken if the remote is untrusted. The function doesn't implement any length
/// checks and an malicious peer might make it consume arbitrary amounts of memory. Checking the
/// `Content-Length` is a possibility, but it is not strictly mandated to be present.
pub async fn aggregate<T>(body: T) -> Result<impl Buf, T::Error>
where
    T: Body,
{
    let mut bufs = BufList::new();

    pin_utils::pin_mut!(body);
    while let Some(buf) = body.data().await {
        let buf = buf?;
        if buf.has_remaining() {
            bufs.push(buf);
        }
    }

    Ok(bufs)
}
