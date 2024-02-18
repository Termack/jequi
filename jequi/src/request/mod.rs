use http::{HeaderMap, HeaderName, HeaderValue};
use std::io::{Error, ErrorKind, Result};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite};

use crate::{HttpConn, Request};

impl<T: AsyncRead + AsyncWrite + Unpin> HttpConn<T> {
    async fn read_until_handle_eof(&mut self, byte: u8, buf: &mut Vec<u8>) -> Result<()> {
        let n = self.stream.read_until(byte, buf).await?;
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
            let next = self.stream.read_u8().await?;
            match next {
                b'\n' => return Ok(()),
                b'\r' => {
                    return (self.stream.read_u8().await? == b'\n')
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

    pub async fn read_body(&mut self) -> Result<()> {
        let content_length: usize = self
            .request
            .get_header("Content-Length")
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
            })?;
        let mut body: Vec<u8> = vec![0; content_length];
        self.stream.read_exact(&mut body).await?;
        self.request.body = Some(body);
        Ok(())
    }
}

impl Request {
    pub fn new() -> Request {
        Request {
            method: String::new(),
            uri: String::new(),
            headers: HeaderMap::new(),
            host: None,
            body: None,
        }
    }

    pub fn get_header(&self, header: &str) -> Option<&HeaderValue> {
        self.headers.get(header.to_lowercase().trim())
    }

    pub fn get_body(&self) -> Option<&[u8]> {
        self.body.as_deref()
    }
}

impl Default for Request {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test;
