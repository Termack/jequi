use futures::Future;
use tokio::io::{AsyncRead, AsyncWrite};

use http::HeaderName;
use std::{
    io::{Error, ErrorKind, Result},
    pin::Pin,
    sync::Arc,
    task::{ready, Context, Poll},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufStream},
    pin,
};

use crate::{body::RequestBody, RawStream, Request};

use super::Http1Conn;

impl<T: AsyncRead + AsyncWrite + Unpin + Send> Http1Conn<T> {
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
        ReadBody {
            content_length: request.get_content_length(),
            conn,
            body: request.body.clone(),
        }
    }
}

pub struct ReadBody<'a, T: AsyncRead + AsyncWrite + Unpin + Send> {
    content_length: Result<usize>,
    conn: &'a mut BufStream<RawStream<T>>,
    body: Arc<RequestBody>,
}

unsafe impl<T: AsyncRead + AsyncWrite + Unpin + Send> Send for ReadBody<'_, T> {}

impl<'a, T: AsyncRead + AsyncWrite + Unpin + Send> Future for ReadBody<'a, T> {
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let content_length: usize = match &self.content_length {
            Ok(content_length) => *content_length,
            Err(err) => {
                self.body.clone().get_mut().write_body(None);
                return Poll::Ready(Err(Error::new(err.kind(), err.to_string())));
            }
        };

        let mut bytes: Vec<u8> = vec![0; content_length];
        let read_exact = self.conn.read_exact(&mut bytes);
        pin!(read_exact);

        match ready!(read_exact.poll(cx)) {
            Ok(_) => {
                self.body.clone().get_mut().write_body(Some(bytes));
                Poll::Ready(Ok(()))
            }
            Err(e) => {
                self.body.clone().get_mut().write_body(None);
                Poll::Ready(Err(e))
            }
        }
    }
}
