use plugins::get_plugin;
use std::{
    io::{IoSlice, Result},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::{
    http1::Http1Conn, http2::conn::Http2Conn, ssl::ssl_new, ConfigMap, HttpConn, RawStream,
};

use crate as jequi;

impl<S: AsyncRead + AsyncWrite + Unpin + Send> AsyncRead for RawStream<S> {
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

impl<S: AsyncRead + AsyncWrite + Unpin + Send> AsyncWrite for RawStream<S> {
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

pub async fn new_http_conn<T: AsyncRead + AsyncWrite + Unpin + Send>(
    stream: T,
    config_map: Arc<ConfigMap>,
) -> HttpConn<T> {
    let plugin_list = &config_map.config;
    let conf = get_plugin!(plugin_list, jequi);

    if conf.tls_active {
        let (stream, version) = ssl_new(stream, config_map.clone()).await;
        if version == "h2" {
            return HttpConn::HTTP2(Http2Conn::new(stream));
        }
        return HttpConn::HTTP1(Http1Conn::new(stream));
    }
    HttpConn::HTTP1(Http1Conn::new(RawStream::Normal(stream)))
}
