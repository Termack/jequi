use chrono::Utc;
use futures::{executor::block_on, Future, FutureExt};
use http::{HeaderMap, HeaderName, HeaderValue};
use std::{
    io::{Error, ErrorKind, Result},
    pin::Pin,
    sync::{Arc, RwLock},
    task::{ready, Context, Poll},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, BufStream},
    pin,
    sync::Mutex,
    time::{sleep, Duration},
};

use crate::{ConfigMap, HttpConn, RawStream, Request, RequestBody, Response};

impl<T: AsyncRead + AsyncWrite + Unpin + Send> HttpConn<T> {
    async fn read_until_handle_eof(&mut self, byte: u8, buf: &mut Vec<u8>) -> Result<()> {
        let n = self.conn.read_until(byte, buf).await?;
        if n == 0 {
            return Err(Error::new(ErrorKind::UnexpectedEof, "unexpected eof"));
        }
        Ok(())
    }

    pub async fn parse_first_line(&mut self) -> Result<()> {
        let mut method = Vec::new();
        let mut uri = Vec::new();
        let mut version = Vec::new();
        self.read_until_handle_eof(b' ', &mut method).await?;
        while uri.is_empty() || uri == [b' '] {
            self.read_until_handle_eof(b' ', &mut uri).await?;
        }
        self.read_until_handle_eof(b'\n', &mut version).await?;

        self.request.method = String::from_utf8_lossy(&method[..method.len() - 1]).to_string();
        self.request.uri = String::from_utf8_lossy(uri.trim_ascii()).to_string();
        self.version = String::from_utf8_lossy(version.trim_ascii()).to_string();
        Ok(())
    }

    pub async fn parse_headers(&mut self) -> Result<()> {
        loop {
            let next = self.conn.read_u8().await?;
            match next {
                b'\n' => return Ok(()),
                b'\r' => {
                    return (self.conn.read_u8().await? == b'\n')
                        .then_some(())
                        .ok_or(Error::new(
                            ErrorKind::InvalidData,
                            "Carriage return was not followed by line feed",
                        ))
                }
                _ => (),
            }
            let mut header = vec![next];
            let mut value = Vec::new();
            self.read_until_handle_eof(b':', &mut header).await?;
            self.read_until_handle_eof(b'\n', &mut value).await?;

            let header = String::from_utf8_lossy(header[..header.len() - 1].trim_ascii_start())
                .to_lowercase();
            let value = String::from_utf8_lossy(value[..value.len() - 1].trim_ascii()).to_string();

            if header == "host" {
                self.request.host = Some(value.clone());
            }
            self.request.headers.insert(
                header.parse::<HeaderName>().unwrap(),
                value.parse().unwrap(),
            );
        }
    }

    pub fn read_body<'b>(
        conn: &'b mut BufStream<RawStream<T>>,
        request: &Request,
    ) -> ReadBody<'b, T> {
        // ReadBody {
        //     content_length: self.request.get_content_length(),
        //     conn: &mut self.conn,
        //     body: &mut self.request.body,
        // }
        ReadBody {
            content_length: request.get_content_length(),
            conn,
            body: request.body.clone(),
        }
    }
}

pub struct ReadBody<'a, T: AsyncRead + AsyncWrite + Unpin> {
    content_length: Result<usize>,
    conn: &'a mut BufStream<RawStream<T>>,
    body: Arc<Mutex<RequestBody>>,
}

impl<'a, T: AsyncRead + AsyncWrite + Unpin> Future for ReadBody<'a, T> {
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let content_length: usize = match &self.content_length {
            Ok(content_length) => *content_length,
            Err(err) => {
                let self_pin = Pin::new(&self);
                println!("deadlock?");
                let lock = self_pin.body.lock();
                pin!(lock);
                let mut body = ready!(lock.poll(cx));
                println!("no");
                body.write_body(None);
                return Poll::Ready(Err(Error::new(err.kind(), err.to_string())));
            }
        };

        let mut bytes: Vec<u8> = vec![0; content_length];
        let mut self_pin = Pin::new(&mut self);
        let read_exact = self_pin.conn.read_exact(&mut bytes);
        pin!(read_exact);
        ready!(read_exact.poll(cx))?;

        let self_pin = Pin::new(&self);
        println!("deadlock?");
        let lock = self_pin.body.lock();
        pin!(lock);
        let mut body = ready!(lock.poll(cx));
        println!("no");
        body.write_body(Some(bytes));
        Poll::Ready(Ok(()))
    }
}

impl Request {
    pub fn new() -> Request {
        Request {
            method: String::new(),
            uri: String::new(),
            headers: HeaderMap::new(),
            host: None,
            body: Arc::new(Mutex::new(RequestBody::default())),
        }
    }

    fn get_content_length(&self) -> Result<usize> {
        self.get_header("Content-Length")
            .ok_or(Error::new(ErrorKind::NotFound, "No content length"))?
            .to_str()
            .map_err(|err| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("Cant convert content length to int: {}", err),
                )
            })?
            .parse()
            .map_err(|err| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("Cant convert content length to int: {}", err),
                )
            })
    }

    pub async fn handle_request(&mut self, response: &mut Response, config_map: Arc<ConfigMap>) {
        response.set_header("server", "jequi");
        response.set_header(
            "date",
            &Utc::now().format("%a, %e %b %Y %T GMT").to_string(),
        );

        let config = config_map.get_config_for_request(self.host.as_deref(), &self.uri);

        for handle_plugin in config.iter().map(|x| &x.request_handler.0).flat_map(|x| x) {
            if let Some(fut) = handle_plugin(self, response) {
                fut.await
            }
        }

        if response.status == 0 {
            response.status = 200;
        }
    }

    pub fn get_header(&self, header: &str) -> Option<&HeaderValue> {
        self.headers.get(header.to_lowercase().trim())
    }

    pub async fn get_body(&self) -> Option<Vec<u8>> {
        let body = self.body.lock().await.get_body().await;
        println!("body was used :O");
        body
    }
}

impl Default for Request {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test;
