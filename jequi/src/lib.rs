#![feature(let_chains)]
#![feature(io_error_more)]
#![feature(option_get_or_insert_default)]
pub mod config;
pub mod request;
pub mod response;
pub mod ssl;
pub mod tcp_stream;

use std::{
    any::Any,
    collections::HashMap,
    fmt::{self, Debug},
    sync::Arc,
};

use futures::future::BoxFuture;
use http::HeaderMap;
use serde::Deserialize;
use serde_yaml::Value;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_openssl::SslStream;
use trait_set::trait_set;

trait_set! {
    pub trait RequestHandlerFn = for<'a> Fn(&'a mut Request, &'a mut Response<'_>) -> Option<BoxFuture<'a, ()>>
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
    pub uri: Option<HashMap<String, ConfigList>>,
    pub config: ConfigList,
}

#[derive(Default, Debug)]
pub struct ConfigMap {
    pub host: Option<HashMap<String, HostConfig>>,
    pub uri: Option<HashMap<String, ConfigList>>,
    pub config: ConfigList,
}

#[derive(Deserialize)]
pub struct HostConfigParser {
    pub uri: Option<HashMap<String, Value>>,
    #[serde(flatten)]
    pub config: Value,
}

#[derive(Deserialize)]
pub struct ConfigMapParser {
    pub host: Option<HashMap<String, HostConfigParser>>,
    pub uri: Option<HashMap<String, Value>>,
    #[serde(flatten)]
    pub config: Value,
}

pub trait JequiConfig: Any + Send + Sync + Debug {
    fn load(config: &Value) -> Option<Self>
    where
        Self: Sized;
    fn as_any(&self) -> &dyn Any;
}

pub fn load_plugin(config: &Value) -> Option<Plugin> {
    let config = Arc::new(Config::load(config)?);
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
    pub headers: HeaderMap,
    pub host: Option<String>,
    pub body: Option<String>,
}

#[repr(C)]
pub struct Response<'a> {
    pub status: usize,
    pub headers: HeaderMap,
    pub body_buffer: &'a mut [u8],
    pub body_length: usize,
}

pub struct HttpConn<'a, T: AsyncRead + AsyncWrite + Unpin> {
    pub raw: RawHTTP<'a, T>,
    pub version: String,
    pub request: Request,
    pub response: Response<'a>,
}
