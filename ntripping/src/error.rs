use std::io;

#[derive(Debug)]
pub enum Error {
    InvalidUri(&'static str),
    Io(io::Error),
    Hyper(hyper::Error),
    Http(hyper::http::Error),
    BadStatus(hyper::StatusCode),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<hyper::Error> for Error {
    fn from(e: hyper::Error) -> Self {
        Self::Hyper(e)
    }
}

impl From<hyper::http::Error> for Error {
    fn from(e: hyper::http::Error) -> Self {
        Self::Http(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUri(msg) => write!(f, "invalid uri: {msg}"),
            Self::Io(e) => write!(f, "io error: {e}"),
            Self::Hyper(e) => write!(f, "hyper error: {e}"),
            Self::Http(e) => write!(f, "http error: {e}"),
            Self::BadStatus(status) => write!(f, "bad status: {status}"),
        }
    }
}

impl std::error::Error for Error {}
