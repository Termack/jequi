#![feature(let_chains)]
#![feature(io_error_more)]
#![feature(option_get_or_insert_default)]
#![feature(byte_slice_trim_ascii)]
pub mod config;
pub mod http2;
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
use tokio::io::{AsyncRead, AsyncWrite, BufStream};
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
}

pub enum RawStream<T: AsyncRead + AsyncWrite + Unpin> {
    Ssl(SslStream<T>),
    Normal(T),
}

pub struct Request {
    pub method: String,
    pub uri: String,
    pub headers: HeaderMap,
    pub host: Option<String>,
    pub body: Option<Vec<u8>>,
}

#[repr(C)]
pub struct Response {
    pub status: usize,
    pub headers: HeaderMap,
    pub body_buffer: Vec<u8>,
}

pub struct HttpConn<T: AsyncRead + AsyncWrite + Unpin> {
    pub stream: BufStream<RawStream<T>>,
    pub version: String,
    pub request: Request,
    pub response: Response,
}
