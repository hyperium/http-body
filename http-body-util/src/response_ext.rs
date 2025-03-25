use crate::combinators::{BoxBody, UnsyncBoxBody};

/// An extension trait for [`http::Response`] adding various combinators and adapters
pub trait ResponseExt<B> {
    /// Returns a new `http::Response` with the body boxed.
    ///
    /// This lets branches return different concrete body types with the same data and error types.
    /// The status, version, headers, and extensions are preserved.
    /// This is equivalent to `response.map(BodyExt::boxed)`.
    ///
    /// # Example
    ///
    /// ```
    /// use bytes::Bytes;
    /// use http::Response;
    /// use http_body_util::{Empty, ResponseExt};
    ///
    /// # let some_condition = true;
    /// let response = if some_condition {
    ///     greeting().box_body()
    /// } else {
    ///     empty().box_body()
    /// };
    ///
    /// fn greeting() -> Response<String> {
    ///     Response::new("Hello, World!".to_string())
    /// }
    ///
    /// fn empty() -> Response<Empty<Bytes>> {
    ///     Response::new(Empty::new())
    /// }
    /// ```
    fn box_body(self) -> http::Response<BoxBody<B::Data, B::Error>>
    where
        B: http_body::Body + Send + Sync + 'static;

    /// Returns a new `http::Response` with the body boxed and !Sync.
    ///
    /// This lets branches return different concrete body types with the same data and error types,
    /// without requiring the body to implement `Sync`.
    /// The status, version, headers, and extensions are preserved.
    /// This is equivalent to `response.map(BodyExt::boxed_unsync)`.
    ///
    /// # Example
    ///
    /// ```
    /// use bytes::Bytes;
    /// use http::Response;
    /// use http_body_util::{Empty, ResponseExt};
    ///
    /// # let some_condition = true;
    /// let response = if some_condition {
    ///     greeting().box_body_unsync()
    /// } else {
    ///     empty().box_body_unsync()
    /// };
    ///
    /// fn greeting() -> Response<String> {
    ///     Response::new("Hello, World!".to_string())
    /// }
    ///
    /// fn empty() -> Response<Empty<Bytes>> {
    ///     Response::new(Empty::new())
    /// }
    /// ```
    fn box_body_unsync(self) -> http::Response<UnsyncBoxBody<B::Data, B::Error>>
    where
        B: http_body::Body + Send + 'static;
}

impl<B> ResponseExt<B> for http::Response<B> {
    fn box_body(self) -> http::Response<BoxBody<B::Data, B::Error>>
    where
        B: http_body::Body + Send + Sync + 'static,
    {
        self.map(crate::BodyExt::boxed)
    }

    fn box_body_unsync(self) -> http::Response<UnsyncBoxBody<B::Data, B::Error>>
    where
        B: http_body::Body + Send + 'static,
    {
        self.map(crate::BodyExt::boxed_unsync)
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use http::{Response, StatusCode, Version};

    use super::*;
    use crate::{BodyExt, Full};

    fn response() -> Response<Full<Bytes>> {
        let mut response = Response::builder()
            .status(StatusCode::CREATED)
            .version(Version::HTTP_2)
            .header("x-test", "preserved")
            .body(Full::new(Bytes::from_static(b"hello")))
            .unwrap();
        response.extensions_mut().insert(42_u32);
        response
    }

    async fn assert_response<B>(response: Response<B>)
    where
        B: http_body::Body<Data = Bytes, Error = std::convert::Infallible>,
    {
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(response.version(), Version::HTTP_2);
        assert_eq!(response.headers()["x-test"], "preserved");
        assert_eq!(response.extensions().get::<u32>(), Some(&42));
        let collected = response.into_body().collect().await.unwrap();
        assert_eq!(collected.to_bytes(), "hello");
    }

    #[tokio::test]
    async fn box_body() {
        assert_response(response().box_body()).await;
    }

    #[tokio::test]
    async fn box_body_unsync() {
        // Cell makes the adapter Send but not Sync.
        let cell = std::cell::Cell::new(0);
        let response = response().map(|body| body.inspect_frame(move |_| cell.set(cell.get() + 1)));
        assert_response(response.box_body_unsync()).await;
    }
}
