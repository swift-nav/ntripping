mod error;
mod header;
pub mod sentence;

use std::{
    convert::Infallible,
    fmt,
    pin::Pin,
    task::{Context, Poll},
};

use bytes::Bytes;
use futures_channel::mpsc;
use futures_util::{Sink, Stream};
use hyper::{
    body::{self, Body, Frame},
    client::conn::http1 as conn,
    Request, StatusCode, Uri,
};
use tokio::net::TcpStream;

use sentence::Sentence;

pub use error::Error;

#[derive(Debug, Default)]
pub struct Client {
    auth: Option<Auth>,
    client_id: Option<String>,
    ntrip_gga: Option<Sentence>,
}

impl Client {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_auth(mut self, auth: impl Into<Option<Auth>>) -> Self {
        self.auth = auth.into();
        self
    }

    pub fn with_client_id(mut self, client_id: impl Into<Option<String>>) -> Self {
        self.client_id = client_id.into();
        self
    }

    pub fn with_ntrip_gga(mut self, ntrip_gga: impl Into<Option<Sentence>>) -> Self {
        self.ntrip_gga = ntrip_gga.into();
        self
    }

    pub async fn connect(&self, uri: Uri) -> Result<Connection, Error> {
        connect(uri, self.headers()).await
    }

    fn headers(&self) -> impl Iterator<Item = (header::HeaderName, String)> {
        let auth = self.auth.as_ref().map(|auth| {
            let auth = base64::encode(format!("{}:{}", auth.username, auth.password));
            (header::AUTHORIZATION, format!("Basic {auth}"))
        });
        let gga = self
            .ntrip_gga
            .map(|gga| (header::NTRIP_GGA, gga.to_string(false)));

        std::iter::once((
            header::SWIFT_CLIENT_ID,
            self.client_id
                .as_deref()
                .unwrap_or("00000000-0000-0000-0000-000000000000")
                .to_owned(),
        ))
        .chain(auth)
        .chain(gga)
    }
}

pub struct Auth {
    username: String,
    password: String,
}

impl Auth {
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }
}

impl fmt::Debug for Auth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Auth")
            .field("username", &self.username)
            .field("password", &"***")
            .finish()
    }
}

#[derive(Debug)]
pub struct Connection {
    send: SendHalf,
    recv: ReceiveHalf,
}

impl Connection {
    pub fn split(self) -> (SendHalf, ReceiveHalf) {
        (self.send, self.recv)
    }
}

impl Stream for Connection {
    type Item = Result<Bytes, ReceiveError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.recv).poll_next(cx)
    }
}

impl<T> Sink<T> for Connection
where
    T: Into<Bytes>,
{
    type Error = SendError;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <SendHalf as Sink<T>>::poll_ready(Pin::new(&mut self.send), cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        <SendHalf as Sink<T>>::start_send(Pin::new(&mut self.send), item)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <SendHalf as Sink<T>>::poll_flush(Pin::new(&mut self.send), cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <SendHalf as Sink<T>>::poll_close(Pin::new(&mut self.send), cx)
    }
}

#[derive(Debug)]
pub struct ReceiveHalf {
    response: hyper::Response<body::Incoming>,
}

impl Stream for ReceiveHalf {
    type Item = Result<Bytes, ReceiveError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(self.response.body_mut()).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                let chunk = frame.into_data();
                if let Some(chunk) = &chunk {
                    tracing::trace!(length = chunk.len(), "recv");
                } else {
                    tracing::trace!("recv eof");
                }
                Poll::Ready(chunk.map(Ok))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(ReceiveError(e)))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[derive(Debug)]
pub struct ReceiveError(hyper::Error);

impl std::fmt::Display for ReceiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "receive error: {}", self.0)
    }
}

impl std::error::Error for ReceiveError {}

#[derive(Debug)]
pub struct SendHalf {
    tx: mpsc::Sender<Bytes>,
}

impl<T> Sink<T> for SendHalf
where
    T: Into<Bytes>,
{
    type Error = SendError;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.tx).poll_ready(cx).map_err(|_| SendError)
    }

    fn start_send(mut self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        Pin::new(&mut self.tx)
            .start_send(item.into())
            .map_err(|_| SendError)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.tx).poll_flush(cx).map_err(|_| SendError)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.tx).poll_close(cx).map_err(|_| SendError)
    }
}

#[derive(Debug)]
pub struct SendError;

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("failed to send message")
    }
}

impl std::error::Error for SendError {}

struct ChannelBody {
    rx: mpsc::Receiver<Bytes>,
}

impl Body for ChannelBody {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match Pin::new(&mut self.rx).poll_next(cx) {
            Poll::Ready(Some(chunk)) => {
                tracing::trace!(length = chunk.len(), "frame ready");
                Poll::Ready(Some(Ok(Frame::data(chunk))))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

async fn connect(
    uri: Uri,
    headers: impl Iterator<Item = (header::HeaderName, String)>,
) -> Result<Connection, Error> {
    let Some(authority) = uri.authority().cloned() else {
        return Err(Error::InvalidUri("invalid authority"));
    };

    let addr = format!("{}:{}", authority.host(), uri.port_u16().unwrap_or(2101));
    tracing::debug!(%addr, "connect");
    let stream = TcpStream::connect(addr).await?;
    let (mut sender, conn) = conn::Builder::new()
        .http09_responses(true)
        .handshake(stream)
        .await?;
    tokio::spawn(conn);

    let mut builder = Request::builder()
        .uri(uri)
        .header(header::HOST, authority.as_str())
        .header(header::USER_AGENT, "NTRIP ntrip-client/1.0")
        .header(header::NTRIP_VERSION, "Ntrip/2.0")
        .method("GET");

    for (key, value) in headers {
        builder = builder.header(key, value);
    }

    if let Some(i) = authority.as_str().find('@') {
        if !builder
            .headers_ref()
            .unwrap()
            .contains_key(&header::AUTHORIZATION)
        {
            let auth = base64::encode(&authority.as_str()[..i]);
            builder = builder.header(header::AUTHORIZATION, format!("Basic {auth}"));
        }
    }

    let (tx, rx) = mpsc::channel::<Bytes>(1);
    let request = builder.body(ChannelBody { rx })?;
    let response = sender.send_request(request).await?;
    if response.status() != StatusCode::OK {
        return Err(Error::BadStatus(response.status()));
    }
    Ok(Connection {
        send: SendHalf { tx },
        recv: ReceiveHalf { response },
    })
}

#[cfg(test)]
mod tests {
    use crate::sentence::Cra;

    use super::*;

    #[test]
    fn client_headers() {
        let client = Client::new();
        assert_eq!(
            client.headers().collect::<Vec<_>>(),
            vec![(
                header::SWIFT_CLIENT_ID,
                "00000000-0000-0000-0000-000000000000".to_string()
            )]
        );

        let client = client.with_client_id("123".to_string());
        assert_eq!(
            client.headers().collect::<Vec<_>>(),
            vec![(header::SWIFT_CLIENT_ID, "123".to_string()),]
        );

        let client = client.with_auth(Auth::new("user", "secret"));
        assert_eq!(
            client.headers().collect::<Vec<_>>(),
            vec![
                (header::SWIFT_CLIENT_ID, "123".to_string()),
                (header::AUTHORIZATION, "Basic dXNlcjpzZWNyZXQ=".to_string())
            ]
        );

        let client = client.with_ntrip_gga(Sentence::CRA(
            Cra::new().with_request_counter(0).with_area_id(1),
        ));
        assert_eq!(
            client.headers().collect::<Vec<_>>(),
            vec![
                (header::SWIFT_CLIENT_ID, "123".to_string()),
                (header::AUTHORIZATION, "Basic dXNlcjpzZWNyZXQ=".to_string()),
                (header::NTRIP_GGA, "$PSWTCRA,0,1,,*51".to_string())
            ]
        );

        let client = client
            .with_auth(None)
            .with_ntrip_gga(None)
            .with_client_id(None);
        assert_eq!(
            client.headers().collect::<Vec<_>>(),
            vec![(
                header::SWIFT_CLIENT_ID,
                "00000000-0000-0000-0000-000000000000".to_string()
            )]
        );
    }

    #[test]
    fn auth_debug_hidden() {
        let auth = format!("{:?}", Auth::new("user", "secret"));
        assert!(!auth.contains("secret"));
    }
}
