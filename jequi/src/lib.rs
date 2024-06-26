#![feature(let_chains)]
#![feature(io_error_more)]
#![feature(option_get_or_insert_default)]
#![feature(byte_slice_trim_ascii)]
#![feature(get_mut_unchecked)]
#![feature(new_uninit)]
pub mod body;
pub mod config;
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

use openssl::pkey::{PKey, Private};

use body::RequestBody;
use futures::future::BoxFuture;
use http::HeaderMap;
use http1::Http1Conn;
use http2::conn::Http2Conn;
use serde::Deserialize;
use serde_yaml::Value;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_openssl::SslStream;
use trait_set::trait_set;

trait_set! {
    pub trait RequestHandlerFn = for<'a> Fn(&'a mut Request, &'a mut Response) -> Option<BoxFuture<'a, ()>>
}

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

pub enum RawStream<T: AsyncRead + AsyncWrite + Unpin + Send> {
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

pub enum HttpConn<T: AsyncRead + AsyncWrite + Unpin + Send> {
    HTTP1(Http1Conn<T>),
    HTTP2(Http2Conn<T>),
}

impl<T: AsyncRead + AsyncWrite + Unpin + Send> HttpConn<T> {
    pub async fn handle_connection(&mut self, config_map: Arc<ConfigMap>) {
        match self {
            HttpConn::HTTP1(ref mut conn) => conn.handle_connection(config_map).await,
            HttpConn::HTTP2(ref mut conn) => conn.handle_connection(config_map).await,
        }
    }
}
