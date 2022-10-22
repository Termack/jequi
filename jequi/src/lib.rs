#![feature(io_error_more)]
pub mod request;
pub mod response;
pub mod ssl;

use std::{
    io::{IoSlice, Result},
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use indexmap::IndexMap;
use tokio_openssl::SslStream;

pub enum RawStream<T: AsyncRead + AsyncWrite + Unpin> {
    Ssl(SslStream<T>),
    Normal(T),
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncRead for RawStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        match *self {
            RawStream::Ssl(ref mut s) => Pin::new(s).poll_read(cx, buf),
            RawStream::Normal(ref mut s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncWrite for RawStream<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        match *self {
            RawStream::Ssl(ref mut s) => Pin::new(s).poll_write(cx, buf),
            RawStream::Normal(ref mut s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize>> {
        match *self {
            RawStream::Ssl(ref mut s) => Pin::new(s).poll_write_vectored(cx, bufs),
            RawStream::Normal(ref mut s) => Pin::new(s).poll_write_vectored(cx, bufs),
        }
    }

    fn is_write_vectored(&self) -> bool {
        match *self {
            RawStream::Ssl(ref s) => s.is_write_vectored(),
            RawStream::Normal(ref s) => s.is_write_vectored(),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        match *self {
            RawStream::Ssl(ref mut s) => Pin::new(s).poll_flush(cx),
            RawStream::Normal(ref mut s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        match *self {
            RawStream::Ssl(ref mut s) => Pin::new(s).poll_shutdown(cx),
            RawStream::Normal(ref mut s) => Pin::new(s).poll_shutdown(cx),
        }
    }
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

#[derive(Default, Debug)]
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

impl<'a, T: AsyncRead + AsyncWrite + Unpin> HttpConn<'a, T> {
    pub async fn new(
        stream: RawStream<T>,
        read_buffer: &'a mut [u8],
        body_buffer: &'a mut [u8],
    ) -> HttpConn<'a, T> {
        HttpConn {
            raw: RawHTTP {
                stream,
                buffer: read_buffer,
                start: 0,
                end: 0,
            },
            version: String::new(),
            request: Request {
                method: String::new(),
                uri: String::new(),
                headers: IndexMap::new(),
            },
            response: Response {
                status: 0,
                headers: IndexMap::new(),
                body_buffer,
                body_length: 0,
            },
        }
    }
}
