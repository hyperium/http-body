use http::HeaderMap;

/// A frame of any kind related to an HTTP stream (body).
#[derive(Debug)]
pub struct Frame<T> {
    kind: Kind<T>,
}

#[derive(Debug)]
enum Kind<T> {
    // The first two variants are "inlined" since they are undoubtedly
    // the most common. This saves us from having to allocate a
    // boxed trait object for them.
    Data(T),
    Trailers(HeaderMap),
    //Unknown(Box<dyn Frameish>),
}

impl<T> Frame<T> {
    /// Create a DATA frame with the provided `Buf`.
    pub fn data(buf: T) -> Self {
        Self {
            kind: Kind::Data(buf),
        }
    }

    /// Create a trailers frame.
    pub fn trailers(map: HeaderMap) -> Self {
        Self {
            kind: Kind::Trailers(map),
        }
    }

    /// Returns whether this is a DATA frame.
    pub fn is_data(&self) -> bool {
        matches!(self.kind, Kind::Data(..))
    }

    /// Consumes self into the buf of the DATA frame.
    ///
    /// Check `Frame::is_data` before to determine if the frame is DATA.
    pub fn into_data(self) -> Option<T> {
        match self.kind {
            Kind::Data(data) => Some(data),
            _ => None,
        }
    }

    /// If this is a DATA frame, returns a reference to it.
    ///
    /// Returns `None` if not a DATA frame.
    pub fn data_ref(&self) -> Option<&T> {
        match self.kind {
            Kind::Data(ref data) => Some(data),
            _ => None,
        }
    }

    /// If this is a DATA frame, returns a mutable reference to it.
    ///
    /// Returns `None` if not a DATA frame.
    pub fn data_mut(&mut self) -> Option<&mut T> {
        match self.kind {
            Kind::Data(ref mut data) => Some(data),
            _ => None,
        }
    }

    /// Returns whether this is a trailers frame.
    pub fn is_trailers(&self) -> bool {
        matches!(self.kind, Kind::Trailers(..))
    }

    /// Consumes self into the buf of the trailers frame.
    ///
    /// Check `Frame::is_trailers` before to determine if the frame is a trailers frame.
    pub fn into_trailers(self) -> Option<HeaderMap> {
        match self.kind {
            Kind::Trailers(trailers) => Some(trailers),
            _ => None,
        }
    }

    /// If this is a trailers frame, returns a reference to it.
    ///
    /// Returns `None` if not a trailers frame.
    pub fn trailers_ref(&self) -> Option<&HeaderMap> {
        match self.kind {
            Kind::Trailers(ref trailers) => Some(trailers),
            _ => None,
        }
    }

    /// If this is a trailers frame, returns a mutable reference to it.
    ///
    /// Returns `None` if not a trailers frame.
    pub fn trailers_mut(&mut self) -> Option<&mut HeaderMap> {
        match self.kind {
            Kind::Trailers(ref mut trailers) => Some(trailers),
            _ => None,
        }
    }
}
