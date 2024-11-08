#![allow(clippy::flat_map_identity)]
#![feature(let_chains)]
#![feature(closure_lifetime_binder)]
#![feature(trait_alias)]
mod client;

use client::Client;
use futures::future::{BoxFuture, FutureExt};
use http::uri::Scheme;
use hyper::body::{self};
use hyper::client::conn;
use hyper_tls::HttpsConnector;
use jequi::hijack::DynAsyncRWSend;
use jequi::http1::Http1Conn;
use jequi::{
    JequiConfig, Plugin, PostRequestHandler, RawStream, Request, RequestHandler, Response, Uri,
};
use openssl::ssl::{Ssl, SslAcceptor, SslConnector, SslMethod};
use rand::seq::SliceRandom;
use serde::{de, Deserialize};
use serde_yaml::Value;
use std::any::Any;
use std::ffi::CStr;
use std::fmt;
use std::ops::Deref;
use std::os::raw::c_char;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::{tcp, TcpStream};

use tokio_openssl::SslStream;

#[no_mangle]
pub unsafe extern "C" fn set_request_uri(req: *mut Request, value: *const c_char) {
    assert!(!req.is_null());
    let req = unsafe { &mut *req };
    let mut uri = unsafe { CStr::from_ptr(value) }
        .to_str()
        .unwrap()
        .to_string();
    if !uri.starts_with('/') {
        uri.insert(0, '/');
    }
    req.uri = Uri::from(uri);
}

pub fn load_plugin(config_yaml: &Value, configs: &mut Vec<Option<Plugin>>) -> Option<Plugin> {
    let config = Config::load(config_yaml, configs)?;
    Some(Plugin {
        config: config.clone(),
        request_handler: RequestHandler(Some(Arc::new(move |req, resp| {
            let config = config.clone(); //TODO: figure out some way to avoid this clone
            config.handle_request(req, resp).boxed()
        }))),
    })
}

impl PartialEq for Config {
    fn eq(&self, other: &Self) -> bool {
        self.proxy_address == other.proxy_address
    }
}

pub trait RequestProxyHandlerFn =
    for<'a> Fn(&'a mut Request, &'a mut Response) -> Option<BoxFuture<'a, Option<String>>>;

pub struct RequestProxyHandler(pub Option<Arc<dyn RequestProxyHandlerFn + Send + Sync>>);

impl fmt::Debug for RequestProxyHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self.0 {
            Some(_) => "fn",
            None => "none",
        };
        write!(f, "{}", text)
    }
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(untagged)]
pub enum ProxyAddress {
    Address(String),
    Addresses(Vec<String>),
}

#[derive(Deserialize, Default, Debug)]
pub struct Config {
    pub proxy_address: Option<ProxyAddress>,
    #[serde(skip)]
    proxy_handlers: Option<Vec<RequestProxyHandler>>,
}

impl Config {
    pub const fn new() -> Self {
        Config {
            proxy_address: None,
            proxy_handlers: None,
        }
    }

    pub fn add_proxy_handler(&mut self, handler: RequestProxyHandler) {
        if self.proxy_handlers.is_none() {
            self.proxy_handlers = Some(Vec::new())
        }
        self.proxy_handlers.as_mut().unwrap().push(handler);
    }

    async fn handle_request(
        self: Arc<Self>,
        req: &mut Request,
        resp: &mut Response,
    ) -> PostRequestHandler {
        let mut proxy_address = None;
        for handle_request in self
            .proxy_handlers
            .iter()
            .flat_map(|x| x)
            .map(|x| &x.0)
            .flat_map(|x| x)
        {
            if let Some(fut) = handle_request(req, resp) {
                if let Some(address) = fut.await {
                    proxy_address = Some(address);
                }
            }
        }

        let mut proxy_address = proxy_address.as_ref();
        if proxy_address.is_none() {
            proxy_address = self.proxy_address.as_ref().map(|a| match a {
                ProxyAddress::Address(address) => address,
                ProxyAddress::Addresses(addresses) => {
                    addresses.choose(&mut rand::thread_rng()).unwrap()
                }
            })
        }

        let mut client = Client::connect(proxy_address).await;

        client.send_request(req).await.unwrap();

        client.get_response(resp).await.unwrap();

        // let url = http::Uri::builder()
        //     .scheme(scheme)
        //     .authority(address)
        //     .path_and_query(req.uri.path())
        //     .build()
        //     .unwrap();

        // let mut request_builder = http::Request::builder().method(req.method.deref()).uri(url);
        // req.headers.insert("Host", address.parse().unwrap());
        // *request_builder.headers_mut().unwrap() = req.headers.clone();
        // let bodyy = req.get_body().await.clone();
        // let body = match bodyy.as_deref() {
        //     None => Body::empty(),
        //     Some(buf) => Body::from(buf.to_owned()),
        // };
        //
        // let (mut request_sender, conn) = conn::handshake(stream).await.unwrap();
        // spawn a task to poll the connection and drive the HTTP state
        // tokio::spawn(async {
        //     if let Err(e) = conn.await {
        //         eprintln!("Error in connection: {}", e);
        //     }
        // });
        //
        // let request = request_builder.body(body).unwrap();
        // let response = request_sender.send_request(request).await.unwrap();
        // resp.status = response.status().as_u16() as usize;
        // resp.headers = response.headers().clone();
        // resp.write_body(&body::to_bytes(response.into_body()).await.unwrap())
        //     .unwrap();

        if let Some(upgrade) = resp.get_header("upgrade") {
            if upgrade == "websocket" {
                let mut server_conn = client.into_conn();
                return PostRequestHandler::HijackConnection(Box::new(
                    move |conn: Http1Conn<Box<dyn DynAsyncRWSend>>| {
                        async move {
                            let mut conn = conn.into_conn();
                            tokio::io::copy_bidirectional(&mut conn, &mut server_conn)
                                .await
                                .unwrap();
                        }
                        .boxed()
                    },
                ));
            }
        }
        PostRequestHandler::Continue
    }
}

impl JequiConfig for Config {
    fn load(config_yaml: &Value, _configs: &mut Vec<Option<Plugin>>) -> Option<Arc<Self>>
    where
        Self: Sized,
    {
        let conf: Config = Deserialize::deserialize(config_yaml).unwrap();
        if conf == Config::default() {
            return None;
        }

        Some(Arc::new(conf))
    }
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
