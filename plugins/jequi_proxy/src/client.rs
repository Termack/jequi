use core::panic;
use http::{
    header::{self},
    HeaderMap,
};
use std::{
    io::{Error, ErrorKind, Result},
    pin::Pin,
};

use http::HeaderName;

use jequi::{AsyncRWSendBuf, RawStream, Request, Response};
use openssl::ssl::{Ssl, SslConnector, SslMethod, SslVerifyMode};
use tokio::net::TcpStream;
use tokio_openssl::SslStream;

use tokio::io::{AsyncReadExt, AsyncWriteExt, BufStream};

use jequi::http1::ReadUntilHandleEof;

pub struct Client<T: AsyncRWSendBuf> {
    host: String,
    conn: T,
}

impl<T: AsyncRWSendBuf> Client<T> {
    pub fn into_conn(self) -> T {
        self.conn
    }

    pub async fn send_request(&mut self, request: &Request) -> Result<()> {
        let mut headers = String::new();
        let first_line = format!("{} {} HTTP/1.1\n", request.method, request.uri.raw());
        headers += &first_line;

        let host_line = format!(
            "{}: {}\n",
            header::HOST,
            self.host.parse::<String>().unwrap()
        );
        headers += &host_line;
        for (key, value) in request.headers.iter() {
            if key == header::HOST {
                continue;
            }
            let header_line = format!("{}: {}\n", key, value.to_str().unwrap());
            headers += &header_line;
        }
        headers += "\n";
        // println!("{}", headers);
        self.conn.write_all(headers.as_bytes()).await?;
        self.conn.flush().await?;

        if let Some(ref body) = *request.body.clone().get_body().await {
            self.conn.write_all(body).await?;
            self.conn.flush().await?;
        }

        Ok(())
    }

    async fn parse_headers(&mut self, headers: &mut HeaderMap) -> Result<()> {
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
            self.conn.read_until_handle_eof(b':', &mut header).await?;
            self.conn.read_until_handle_eof(b'\n', &mut value).await?;

            let header = String::from_utf8_lossy(header[..header.len() - 1].trim_ascii_start())
                .to_lowercase();
            let value = String::from_utf8_lossy(value[..value.len() - 1].trim_ascii()).to_string();

            headers.insert(
                header.parse::<HeaderName>().unwrap(),
                value.parse().unwrap(),
            );
        }
    }

    async fn parse_body(&mut self, response: &mut Response) -> Result<()> {
        let content_length = response.headers.get(header::CONTENT_LENGTH);
        let transfer_encoding = response.headers.get(header::TRANSFER_ENCODING);

        match (content_length, transfer_encoding) {
            (_, Some(transfer_encoding)) => {
                if transfer_encoding != "chunked" {
                    return Err(Error::new(ErrorKind::Unsupported, "encoding not supported"));
                }

                loop {
                    let mut chunk_size = Vec::new();
                    self.conn
                        .read_until_handle_eof(b'\n', &mut chunk_size)
                        .await?;

                    let chunk_size = usize::from_str_radix(
                        String::from_utf8_lossy(&chunk_size).trim_end_matches("\r\n"),
                        16,
                    )
                    .map_err(|err| Error::new(ErrorKind::InvalidData, err))?;

                    let mut buf = vec![0; chunk_size];
                    self.conn.read_exact(&mut buf).await?;

                    response.write_body(&buf)?;

                    let mut buf = vec![0; 2];
                    self.conn.read_exact(&mut buf).await?;

                    if buf != b"\r\n" {
                        return Err(Error::new(
                            ErrorKind::InvalidData,
                            "chunk doesn't end with \\r\\n",
                        ));
                    }

                    if chunk_size == 0 {
                        return Ok(());
                    }
                }
            }
            (Some(content_length), None) => {
                let content_length = content_length
                    .to_str()
                    .map_err(|err| Error::new(ErrorKind::InvalidData, err))?
                    .parse()
                    .map_err(|err| Error::new(ErrorKind::InvalidData, err))?;
                let mut bytes: Vec<u8> = vec![0; content_length];
                self.conn.read_exact(&mut bytes).await?;
                response.body_buffer = bytes;
            }
            (None, None) => (),
        }

        Ok(())
    }

    pub async fn get_response(&mut self, response: &mut Response) -> Result<()> {
        let mut version = Vec::new();
        let mut status_text = Vec::new();
        self.conn.read_until_handle_eof(b' ', &mut version).await?;
        let mut status: Vec<u8> = vec![0; 3];
        self.conn.read_exact(&mut status).await?;
        self.conn
            .read_until_handle_eof(b'\n', &mut status_text)
            .await?;

        response.status = String::from_utf8(status)
            .map_err(|err| Error::new(ErrorKind::InvalidData, err))?
            .parse()
            .map_err(|err| Error::new(ErrorKind::InvalidData, err))?;

        self.parse_headers(&mut response.headers).await?;

        self.parse_body(response).await?;

        Ok(())
    }
}

impl Client<BufStream<RawStream<TcpStream>>> {
    pub async fn connect(
        proxy_address: Option<&String>,
    ) -> Client<BufStream<RawStream<TcpStream>>> {
        let (host, port, scheme) = {
            let proxy_address = proxy_address.unwrap();

            let parts: Vec<&str> = proxy_address.splitn(2, "://").collect();

            // Get scheme from proxy_address or https as default
            let (address, scheme) = match parts.len() {
                2 => (parts[1], parts[0]),
                1 => (parts[0], "https"),
                _ => panic!("invalid address"),
            };

            let parts: Vec<&str> = address.splitn(2, ":").collect();

            let (host, port) = match parts.len() {
                2 => (parts[0], parts[1].parse::<u16>().unwrap()),
                1 => (
                    parts[0],
                    match scheme {
                        "https" => 443,
                        _ => 80,
                    },
                ),
                _ => panic!("invalid address"),
            };
            (host, port, scheme)
        };

        let stream = TcpStream::connect((host, port)).await.unwrap();

        let host = "simple-retro.ephemeral.dev.br";

        let conn = match scheme {
            "https" => {
                let mut connector = SslConnector::builder(SslMethod::tls()).unwrap();
                connector.set_verify(SslVerifyMode::NONE);
                let connector = connector.build();
                let mut ssl = Ssl::new(connector.context()).unwrap();
                ssl.set_hostname(host).unwrap();

                let mut stream = SslStream::new(ssl, stream).unwrap();
                Pin::new(&mut stream).connect().await.unwrap();
                RawStream::Ssl(stream)
            }
            _ => RawStream::Normal(stream),
        };

        Client {
            host: host.to_string(),
            conn: BufStream::new(conn),
        }
    }
}
