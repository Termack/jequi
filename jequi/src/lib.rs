#![feature(io_error_more)]
pub mod request;
pub mod response;
pub mod ssl;
pub mod config;
pub mod tcp_stream;

use tokio::io::{AsyncRead, AsyncWrite};
use indexmap::IndexMap;
use tokio_openssl::SslStream;
use serde::Deserialize;

#[derive(Deserialize, Debug, PartialEq)]
#[serde(default,deny_unknown_fields)]
pub struct Config {
    pub ip: String,
    pub port: u16,
    pub static_files_path: Option<String>,
    pub tls_active: bool,
    pub go_handler_path: Option<String>,
    #[serde(skip)]
    pub go_library_path: Option<String>
}

pub enum RawStream<T: AsyncRead + AsyncWrite + Unpin> {
    Ssl(SslStream<T>),
    Normal(T),
}

pub struct RawHTTP<'a, T: AsyncRead + AsyncWrite + Unpin> {
    pub stream: RawStream<T>,
    pub buffer: &'a mut [u8],
    pub start: usize,
    pub end: usize,
}

pub struct Request {
    pub method: String,
    pub uri: String,
    pub headers: IndexMap<String, String>,
}

#[repr(C)]
pub struct Response<'a> {
    pub status: usize,
    pub headers: IndexMap<String, String>,
    pub body_buffer: &'a mut [u8],
    pub body_length: usize,
}

pub struct HttpConn<'a, T: AsyncRead + AsyncWrite + Unpin> {
    pub raw: RawHTTP<'a, T>,
    pub version: String,
    pub request: Request,
    pub response: Response<'a>,
}