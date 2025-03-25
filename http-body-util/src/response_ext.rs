use crate::combinators::{BoxBody, UnsyncBoxBody};

/// An extension trait for [`http::Request`] adding various combinators and adapters
pub trait ResponseExt<B> {
    /// Returns a new `http::Response` with the body boxed.
    ///
    /// This is useful when you have need to return a Response where the body type is not known at
    /// compile time.
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
    /// This is useful when you have need to return a Response where the body type is not known at
    /// compile time and the body is not Sync.
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
    use http::{Response, StatusCode, Uri};

    use super::*;
    use crate::{Empty, Full};

    #[test]
    fn box_body() {
        let uri: Uri = "http://example.com".parse().unwrap();
        let _response = match uri.path() {
            "/greeting" => greeting().box_body(),
            "/empty" => empty().box_body(),
            _ => not_found().box_body(),
        };
    }

    fn greeting() -> Response<String> {
        Response::new("Hello, World!".to_string())
    }

    fn empty() -> Response<Empty<Bytes>> {
        Response::new(Empty::new())
    }

    fn not_found() -> Response<Full<Bytes>> {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body("Not Found".into())
            .unwrap()
    }
}
