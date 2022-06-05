use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Buf;
use http::HeaderMap;
use http_body::{Body, SizeHint};
use proj::EitherProj;

/// sum type with two cases: `Left` and `Right`, used if a body can be one of two distinct types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Either<L, R> {
    /// A value of type `L`
    Left(L),
    /// A value of type `R`
    Right(R),
}

impl<L, R> Either<L, R> {
    pub(crate) fn project(self: Pin<&mut Self>) -> EitherProj<L, R> {
        unsafe {
            match self.get_unchecked_mut() {
                Self::Left(left) => EitherProj::Left(Pin::new_unchecked(left)),
                Self::Right(right) => EitherProj::Right(Pin::new_unchecked(right)),
            }
        }
    }

    /// Flip the values, `Left` -> `Right` and `Right` -> `Left`
    pub fn flip(self) -> Either<R, L> {
        match self {
            Either::Left(left) => Either::Right(left),
            Either::Right(right) => Either::Left(right),
        }
    }

    /// Apply the function `f` to the left variant, if present.
    pub fn map_left<F: FnOnce(L) -> T, T>(self, f: F) -> Either<T, R> {
        match self {
            Either::Left(left) => Either::Left(f(left)),
            Either::Right(right) => Either::Right(right),
        }
    }

    /// Apply the function `g` to the right variant, if present.
    pub fn map_right<F: FnOnce(R) -> T, T>(self, f: F) -> Either<L, T> {
        match self {
            Either::Left(left) => Either::Left(left),
            Either::Right(right) => Either::Right(f(right)),
        }
    }

    /// Apply the function `f` to the left variant, or the function `g` to the right variant.
    pub fn map<F: FnOnce(L) -> T, T, G: FnOnce(R) -> U, U>(self, f: F, g: G) -> Either<T, U> {
        match self {
            Either::Left(left) => Either::Left(f(left)),
            Either::Right(right) => Either::Right(g(right)),
        }
    }

    /// Apply, depending on the current variant, the function `f`
    /// on the left variant or the function `g` on the right variant and return their result.
    pub fn either<F: FnOnce(L) -> T, G: FnOnce(R) -> T, T>(self, f: F, g: G) -> T {
        match self {
            Either::Left(left) => f(left),
            Either::Right(right) => g(right),
        }
    }

    /// Convert `&Either<L, R>` into `Either<&L, &R>`
    pub fn as_ref(&self) -> Either<&L, &R> {
        match self {
            Either::Left(left) => Either::Left(left),
            Either::Right(right) => Either::Right(right),
        }
    }
}

impl<L> Either<L, L> {
    /// Convert [`Either`] into the inner type, if both `Left` and `Right` are of the same type.
    pub fn into_inner(self) -> L {
        match self {
            Either::Left(left) => left,
            Either::Right(right) => right,
        }
    }
}

impl<L, R> From<Result<L, R>> for Either<L, R> {
    fn from(value: Result<L, R>) -> Self {
        match value {
            Ok(ok) => Either::Left(ok),
            Err(err) => Either::Right(err),
        }
    }
}

impl<L, R> From<Either<L, R>> for Result<L, R> {
    fn from(value: Either<L, R>) -> Self {
        match value {
            Either::Left(left) => Ok(left),
            Either::Right(right) => Err(right),
        }
    }
}

impl<L: Display, R: Display> Display for Either<L, R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Either::Left(left) => Display::fmt(left, f),
            Either::Right(right) => Display::fmt(right, f),
        }
    }
}

impl<L: Error, R: Error> Error for Either<L, R> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Either::Left(left) => left.source(),
            Either::Right(right) => right.source(),
        }
    }
}

impl<L: Buf, R: Buf> Buf for Either<L, R> {
    fn remaining(&self) -> usize {
        match self {
            Either::Left(left) => left.remaining(),
            Either::Right(right) => right.remaining(),
        }
    }

    fn chunk(&self) -> &[u8] {
        match self {
            Either::Left(left) => left.chunk(),
            Either::Right(right) => right.chunk(),
        }
    }

    fn advance(&mut self, cnt: usize) {
        match self {
            Either::Left(left) => left.advance(cnt),
            Either::Right(right) => right.advance(cnt),
        }
    }
}

impl<L: Body, R: Body> Body for Either<L, R> {
    type Data = Either<L::Data, R::Data>;
    type Error = Either<L::Error, R::Error>;

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        match self.project() {
            EitherProj::Left(left) => left
                .poll_data(cx)
                .map(|poll| poll.map(|opt| opt.map(Either::Left).map_err(Either::Left))),
            EitherProj::Right(right) => right
                .poll_data(cx)
                .map(|poll| poll.map(|opt| opt.map(Either::Right).map_err(Either::Right))),
        }
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        match self.project() {
            EitherProj::Left(left) => left
                .poll_trailers(cx)
                .map(|poll| poll.map_err(Either::Left)),
            EitherProj::Right(right) => right
                .poll_trailers(cx)
                .map(|poll| poll.map_err(Either::Right)),
        }
    }

    fn is_end_stream(&self) -> bool {
        match self {
            Either::Left(left) => left.is_end_stream(),
            Either::Right(right) => right.is_end_stream(),
        }
    }

    fn size_hint(&self) -> SizeHint {
        match self {
            Either::Left(left) => left.size_hint(),
            Either::Right(right) => right.size_hint(),
        }
    }
}

pub(crate) mod proj {
    //! This code is the (cleaned output) generated by [pin-project-lite], as it
    //! does not support tuple variants.
    //!
    //! This is the altered expansion from the following snippet, expanded by `cargo-expand`:
    //! ```rust
    //! use pin_project_lite::pin_project;
    //!
    //! pin_project! {
    //!     #[project = EitherProj]
    //!     pub enum Either<L, R> {
    //!         Left {#[pin] left: L},
    //!         Right {#[pin] right: R}
    //!     }
    //! }
    //! ```  
    //!
    //! [pin-project-lite]: https://docs.rs/pin-project-lite/latest/pin_project_lite/
    use std::marker::PhantomData;
    use std::pin::Pin;

    use super::Either;

    #[allow(dead_code)]
    #[allow(single_use_lifetimes)]
    #[allow(unknown_lints)]
    #[allow(clippy::mut_mut)]
    #[allow(clippy::redundant_pub_crate)]
    #[allow(clippy::ref_option_ref)]
    #[allow(clippy::type_repetition_in_bounds)]
    pub(crate) enum EitherProj<'__pin, L, R>
    where
        Either<L, R>: '__pin,
    {
        Left(Pin<&'__pin mut L>),
        Right(Pin<&'__pin mut R>),
    }

    #[allow(single_use_lifetimes)]
    #[allow(unknown_lints)]
    #[allow(clippy::used_underscore_binding)]
    #[allow(missing_debug_implementations)]
    const _: () = {
        #[allow(non_snake_case)]
        pub struct __Origin<'__pin, L, R> {
            __dummy_lifetime: PhantomData<&'__pin ()>,
            Left: L,
            Right: R,
        }
        impl<'__pin, L, R> Unpin for Either<L, R> where __Origin<'__pin, L, R>: Unpin {}

        trait MustNotImplDrop {}
        #[allow(drop_bounds)]
        impl<T: Drop> MustNotImplDrop for T {}
        impl<L, R> MustNotImplDrop for Either<L, R> {}
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Empty, Full};

    #[tokio::test]
    async fn data_left() {
        let full = Full::new(&b"hello"[..]);

        let mut value: Either<_, Empty<&[u8]>> = Either::Left(full);

        assert_eq!(value.size_hint().exact(), Some(b"hello".len() as u64));
        assert_eq!(value.data().await, Some(Ok(Either::Left(&b"hello"[..]))));
        assert!(value.data().await.is_none());
    }

    #[tokio::test]
    async fn data_right() {
        let full = Full::new(&b"hello!"[..]);

        let mut value: Either<Empty<&[u8]>, _> = Either::Right(full);

        assert_eq!(value.size_hint().exact(), Some(b"hello!".len() as u64));
        assert_eq!(value.data().await, Some(Ok(Either::Right(&b"hello!"[..]))));
        assert!(value.data().await.is_none());
    }

    #[test]
    fn flip() {
        let a = 2;
        let b = "example";

        assert_eq!(Either::<i32, &str>::Left(a).flip(), Either::Right(a));
        assert_eq!(Either::<i32, &str>::Right(b).flip(), Either::Left(b));
    }

    #[test]
    fn map_left() {
        let a = 2;
        let b = "example";

        assert_eq!(
            Either::<i32, &str>::Left(a).map_left(|a| a + 2),
            Either::Left(4)
        );
        assert_eq!(
            Either::<i32, &str>::Right(b).map_left(|a| a + 2),
            Either::Right(b)
        );
    }

    #[test]
    fn map_right() {
        let a = 2;
        let b = "example";

        assert_eq!(
            Either::<i32, &str>::Left(a).map_right(|_| "hi"),
            Either::Left(2)
        );
        assert_eq!(
            Either::<i32, &str>::Right(b).map_right(|_| "hi"),
            Either::Right("hi")
        );
    }

    #[test]
    fn map() {
        let a = 2;
        let b = "example";

        assert_eq!(
            Either::<i32, &str>::Left(a).map(|a| a + 2, |_| "hi"),
            Either::Left(4)
        );
        assert_eq!(
            Either::<i32, &str>::Right(b).map(|a| a + 2, |_| "hi"),
            Either::Right("hi")
        );
    }

    #[test]
    fn either() {
        let a = 2;
        let b = "example";

        assert_eq!(Either::<usize, &str>::Left(a).either(|a| a, |b| b.len()), a);
        assert_eq!(
            Either::<usize, &str>::Right(b).either(|a| a, |b| b.len()),
            b.len()
        );
    }

    #[test]
    fn as_ref() {
        let a = &Either::<i32, u8>::Left(2);

        assert_eq!(a.as_ref(), Either::<&i32, &u8>::Left(&2));
    }

    #[test]
    fn into_inner() {
        let a = Either::<i32, i32>::Left(2);
        assert_eq!(a.into_inner(), 2)
    }
}
