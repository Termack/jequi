#![feature(closure_lifetime_binder)]
use futures::future::{BoxFuture, FutureExt};
use hyper::body;
use hyper::{Body, Client};
use hyper_tls::HttpsConnector;
use jequi::{JequiConfig, Plugin, Request, RequestHandler, Response};
use serde::Deserialize;
use serde_yaml::Value;
use std::any::Any;
use std::ffi::CStr;
use std::fmt;
use std::ops::Deref;
use std::os::raw::c_char;
use std::sync::Arc;
use trait_set::trait_set;

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
    req.uri = uri;
}

pub fn load_plugin(config_yaml: &Value, configs: &mut Vec<Option<Plugin>>) -> Option<Plugin> {
    let config = Config::load(config_yaml, configs)?;
    Some(Plugin {
        config: config.clone(),
        request_handler: RequestHandler(Some(Arc::new(move |req, resp| {
            let config = config.clone(); //TODO: figure out some way to avoid this clone
            Some(async move { config.handle_request(req, resp).await }.boxed())
        }))),
    })
}

impl PartialEq for Config {
    fn eq(&self, other: &Self) -> bool {
        self.proxy_address == other.proxy_address
    }
}

trait_set! {
    pub trait RequestProxyHandlerFn = for<'a> Fn(&'a mut Request, &'a mut Response) -> Option<BoxFuture<'a, Option<String>>>
}

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

#[derive(Deserialize, Default, Debug)]
pub struct Config {
    pub proxy_address: Option<String>,
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

    async fn handle_request(&self, req: &mut Request, resp: &mut Response) {
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

        let mut proxy_address = &proxy_address;
        if proxy_address.is_none() {
            proxy_address = &self.proxy_address;
        }

        let url = http::Uri::builder()
            .scheme("https")
            .authority(proxy_address.as_ref().unwrap().deref())
            .path_and_query(req.uri.deref())
            .build()
            .unwrap();
        let mut request_builder = http::Request::builder().method(req.method.deref()).uri(url);
        req.headers.insert(
            "Host",
            proxy_address.as_ref().unwrap().deref().parse().unwrap(),
        );
        *request_builder.headers_mut().unwrap() = req.headers.clone();
        let body = match req.body.as_ref() {
            Some(body) => Body::from(body.clone()),
            None => Body::empty(),
        };
        let https = HttpsConnector::new();
        let client = Client::builder().build::<_, Body>(https);
        let request = request_builder.body(body).unwrap();
        let response = client.request(request).await.unwrap();
        resp.headers = response.headers().clone();
        resp.write_body(&body::to_bytes(response.into_body()).await.unwrap())
            .unwrap();
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
