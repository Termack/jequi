#![feature(let_chains)]
#![feature(io_error_more)]
#![feature(option_get_or_insert_default)]
#![feature(get_mut_unchecked)]
#![feature(new_uninit)]
#![feature(type_alias_impl_trait)]
#![feature(trait_alias)]
pub mod body;
pub mod config;
pub mod hijack;
pub mod http1;
pub mod http2;
pub mod request;
pub mod response;
pub mod ssl;
pub mod tcp_stream;

use std::{
    any::Any,
    collections::HashMap,
    fmt::{self, Debug},
    path::PathBuf,
    sync::Arc,
};

use body::RequestBody;
use futures::future::BoxFuture;
use http::HeaderMap;
use http1::Http1Conn;
use http2::conn::Http2Conn;
use plugins::get_plugin;
use serde::Deserialize;
use serde_yaml::Value;
use ssl::ssl_new;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, BufStream};
use tokio_openssl::SslStream;

use crate as jequi;

pub use hijack::PostRequestHandler;

pub trait RequestHandlerFn =
    for<'a> Fn(&'a mut Request, &'a mut Response) -> BoxFuture<'a, PostRequestHandler>;

pub trait AsyncRWSend = AsyncBufRead + AsyncRead + AsyncWrite + Unpin + Send + 'static;

pub struct RequestHandler(pub Option<Arc<dyn RequestHandlerFn + Send + Sync>>);

impl Debug for RequestHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self.0 {
            Some(_) => "fn",
            None => "none",
        };
        write!(f, "{}", text)
    }
}
#[derive(Debug)]
pub struct Plugin {
    pub config: Arc<dyn JequiConfig>,
    pub request_handler: RequestHandler,
}

pub type ConfigList = Vec<Plugin>;

#[derive(Debug)]
pub struct HostConfig {
    pub path: Option<HashMap<PathBuf, ConfigList>>,
    pub config: ConfigList,
}

#[derive(Default, Debug)]
pub struct ConfigMap {
    pub host: Option<HashMap<String, HostConfig>>,
    pub path: Option<HashMap<PathBuf, ConfigList>>,
    pub config: ConfigList,
}

#[derive(Deserialize)]
pub struct HostConfigParser {
    pub path: Option<HashMap<PathBuf, Value>>,
    #[serde(flatten)]
    pub config: Value,
}

#[derive(Deserialize)]
pub struct ConfigMapParser {
    pub host: Option<HashMap<String, HostConfigParser>>,
    pub path: Option<HashMap<PathBuf, Value>>,
    #[serde(flatten)]
    pub config: Value,
}

pub trait JequiConfig: Any + Send + Sync + Debug {
    fn load(config_yaml: &Value, configs: &mut Vec<Option<Plugin>>) -> Option<Arc<Self>>
    where
        Self: Sized;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub fn load_plugin(config_yaml: &Value, configs: &'_ mut Vec<Option<Plugin>>) -> Option<Plugin> {
    let config = Config::load(config_yaml, configs)?;
    Some(Plugin {
        config: config.clone(),
        request_handler: RequestHandler(None),
    })
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(default)]
pub struct Config {
    pub ip: String,
    pub port: u16,
    pub tls_active: bool,
    pub http2: bool,
    pub chunk_size: usize,
    pub ssl_key: Option<ssl::SslKeyConfig>,
    pub ssl_certificate: Option<ssl::SslCertConfig>,
}

pub enum RawStream<T: AsyncRWSend> {
    Ssl(SslStream<T>),
    Normal(T),
}

pub struct Uri(String);

pub struct Request {
    pub method: String,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub host: Option<String>,
    pub body: Arc<RequestBody>,
}

#[repr(C)]
pub struct Response {
    pub status: usize,
    pub headers: HeaderMap,
    pub body_buffer: Vec<u8>,
}

pub enum HttpConn<T: AsyncRead + AsyncWrite + Unpin + Send + 'static> {
    HTTP1(Http1Conn<BufStream<T>>),
    HTTP1Ssl(Http1Conn<BufStream<SslStream<T>>>),
    HTTP2(Http2Conn<BufStream<SslStream<T>>>),
}

impl<T: AsyncRead + AsyncWrite + Unpin + Send + 'static> HttpConn<T> {
    pub async fn new(stream: T, config_map: Arc<ConfigMap>) -> HttpConn<T> {
        let plugin_list = &config_map.config;
        let conf = get_plugin!(plugin_list, jequi).unwrap();

        if conf.tls_active {
            let (stream, version) = ssl_new(stream, config_map.clone()).await;
            if version == "h2" {
                return HttpConn::HTTP2(Http2Conn::new(stream));
            }
            return HttpConn::HTTP1Ssl(Http1Conn::new(stream));
        }
        HttpConn::HTTP1(Http1Conn::new(stream))
    }

    pub async fn handle_connection(self, config_map: Arc<ConfigMap>) {
        match self {
            HttpConn::HTTP1(conn) => conn.handle_connection(config_map).await,
            HttpConn::HTTP1Ssl(conn) => conn.handle_connection(config_map).await,
            HttpConn::HTTP2(conn) => conn.handle_connection(config_map).await,
        }
    }
}
