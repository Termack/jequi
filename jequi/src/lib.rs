#![feature(io_error_more)]
pub mod config;
pub mod request;
pub mod response;
pub mod ssl;
pub mod tcp_stream;

use std::{any::Any, sync::Arc, collections::HashMap, fmt::Debug};

use indexmap::IndexMap;
use serde::Deserialize;
use serde_yaml::Value;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_openssl::SslStream;

#[derive(Debug)]
pub struct Plugin {
    pub config: Arc<dyn JequiConfig>,
    pub request_handler: Option<Arc<dyn RequestHandler>>
}

#[derive(Debug)]
pub struct HostConfig {
    pub uri: Option<HashMap<String,Vec<Plugin>>>,
    pub config: Vec<Plugin>
}

#[derive(Default, Debug)]
pub struct ConfigMap {
    pub host: Option<HashMap<String, HostConfig>>,
    pub uri: Option<HashMap<String, Vec<Plugin>>>,
    pub config: Vec<Plugin>
}

#[derive(Deserialize)]
pub struct HostConfigParser {
    pub uri: Option<HashMap<String,Value>>,
    #[serde(flatten)]
    pub config: Value
}

#[derive(Deserialize)]
pub struct ConfigMapParser {
    pub host: Option<HashMap<String, HostConfigParser>>,
    pub uri: Option<HashMap<String, Value>>,
    #[serde(flatten)]
    pub config: Value
}

pub trait RequestHandler: Send + Sync + Debug
{
    fn handle_request(&self, req: &mut Request, resp: &mut Response);
}

pub trait JequiConfig: Any +  Send + Sync + Debug
{
    fn load(config: &Value) -> Option<Self> where Self: Sized;
    fn as_any(&self) -> &dyn Any;
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
    pub headers: IndexMap<String, String>,
    pub host: Option<String>,
    pub body: Option<String>,
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
