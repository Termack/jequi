#![feature(io_error_more)]
pub mod config;
pub mod request;
pub mod response;
pub mod ssl;
pub mod tcp_stream;

use std::{any::Any, collections::HashMap, sync::Arc};

use dyn_clone::DynClone;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_openssl::SslStream;

pub struct Plugin {
    pub config: Box<dyn JequiConfig>,
    pub request_handler: Box<dyn RequestHandler>
}

impl Plugin {
    pub const fn new(config: Box<dyn JequiConfig>,request_handler: Box<dyn RequestHandler>) -> Self {
        Plugin { config,request_handler }
    }
}

pub type ConfigMap = HashMap<String, Value>;

#[derive(Clone, Default)]
pub struct MainConf {
    pub config_map: ConfigMap,
    pub request_handlers: Vec<Box<dyn RequestHandler>>,
}

pub trait RequestHandler: DynClone + Send + Sync
{
    fn handle_request(&self, req: &mut Request, resp: &mut Response);
}
dyn_clone::clone_trait_object!(RequestHandler);

pub trait JequiConfig: Any + DynClone +  Send + Sync
{
    fn load(config: &mut ConfigMap, handlers: &mut Vec<Arc<dyn RequestHandler>>) -> Option<Arc<Self>> where Self: Sized;
    fn as_any(&self) -> &dyn Any;
}
dyn_clone::clone_trait_object!(JequiConfig);


#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
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
