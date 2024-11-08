#![allow(clippy::flat_map_identity)]
mod read;
mod write;
use std::{
    io::{Error, ErrorKind, Result},
    sync::Arc,
};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, BufStream};

use crate::hijack::DynAsyncRWSend;
use crate::{AsyncRWSend, AsyncRWSendBuf, ConfigMap, PostRequestHandler, Request, Response};

use plugins::get_plugin;

use crate::Config;

use crate as jequi;

pub trait ReadUntilHandleEof {
    async fn read_until_handle_eof(&mut self, byte: u8, buf: &mut Vec<u8>) -> Result<()>;
}

impl<T: AsyncRWSendBuf> ReadUntilHandleEof for T {
    async fn read_until_handle_eof(&mut self, byte: u8, buf: &mut Vec<u8>) -> Result<()> {
        let n = self.read_until(byte, buf).await?;
        if n == 0 {
            return Err(Error::new(ErrorKind::UnexpectedEof, "unexpected eof"));
        }
        Ok(())
    }
}

pub struct Http1Conn<T: AsyncRWSendBuf> {
    pub conn: T,
    pub version: String,
    pub request: Request,
    pub response: Response,
}

impl<T: AsyncRWSend> Http1Conn<BufStream<T>> {
    pub fn new(stream: T) -> Http1Conn<BufStream<T>> {
        Http1Conn {
            conn: BufStream::new(stream),
            version: String::new(),
            request: Request::new(),
            response: Response::new(),
        }
    }
}

impl<T: AsyncRWSendBuf> Http1Conn<T> {
    pub fn as_dyn(self) -> Http1Conn<Box<dyn DynAsyncRWSend>> {
        Http1Conn {
            conn: Box::new(self.conn),
            version: self.version,
            request: self.request,
            response: self.response,
        }
    }

    pub fn into_conn(self) -> T {
        self.conn
    }

    pub async fn handle_connection(mut self, config_map: Arc<ConfigMap>) {
        let plugin_list = &config_map.config;
        let conf = get_plugin!(plugin_list, jequi).unwrap();

        let post_handler = self.handle_request(conf, config_map.clone()).await;
        if let PostRequestHandler::HijackConnection(hijack_connection) = post_handler {
            hijack_connection(self.as_dyn()).await;
            return;
        }
        if let Some(connection) = self.request.headers.get("connection")
            && connection.to_str().unwrap().to_lowercase() == "keep-alive"
        {
            loop {
                self.request = Request::new();
                self.response = Response::new();
                self.handle_request(conf, config_map.clone()).await;
            }
        }
    }

    async fn handle_request(
        &mut self,
        conf: &Config,
        config_map: Arc<ConfigMap>,
    ) -> PostRequestHandler {
        self.parse_first_line().await.unwrap();

        self.parse_headers().await.unwrap();

        // TODO: Read the body only if needed (remember to consume stream if body not read)
        let read_body = Http1Conn::read_body(&mut self.conn, &self.request);

        let request = &mut self.request;
        let mut post_handler = PostRequestHandler::Continue;
        tokio_scoped::scope(|scope| {
            scope.spawn(async move {
                match read_body.await {
                    Ok(_) => (),
                    Err(ref e) if e.kind() == ErrorKind::NotFound => (),
                    Err(e) => panic!("Error reading request body: {}", e),
                };
            });

            scope.spawn(async {
                post_handler = request.handle_request(&mut self.response, config_map).await;
            });
        });

        self.write_response(conf.chunk_size).await.unwrap();

        post_handler
    }
}

#[cfg(test)]
mod test;
