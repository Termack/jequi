use http::version;
use std::{
    io::{IoSlice, Result},
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, BufStream, ReadBuf};

use crate::{HttpConn, RawStream, Request, Response};

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncRead for RawStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        match *self {
            RawStream::Ssl(ref mut stream) => Pin::new(stream).poll_read(cx, buf),
            RawStream::Normal(ref mut stream) => Pin::new(stream).poll_read(cx, buf),
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

impl<T: AsyncRead + AsyncWrite + Unpin> HttpConn<T> {
    pub fn with_version(stream: RawStream<T>, version: String) -> HttpConn<T> {
        HttpConn {
            version,
            ..HttpConn::new(stream)
        }
    }

    pub fn new(stream: RawStream<T>) -> HttpConn<T> {
        HttpConn {
            stream: BufStream::new(stream),
            version: String::new(),
            request: Request::new(),
            response: Response::new(),
        }
    }
}
