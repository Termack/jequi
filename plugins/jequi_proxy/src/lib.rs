#![feature(closure_lifetime_binder)]
use futures::future::FutureExt;
use hyper::body;
use hyper::{Body, Client};
use hyper_tls::HttpsConnector;
use jequi::{JequiConfig, Plugin, Request, RequestHandler, Response};
use serde::Deserialize;
use serde_yaml::Value;
use std::any::Any;
use std::ops::Deref;
use std::sync::Arc;

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

#[derive(Deserialize, Default, Debug)]
pub struct Config {
    pub proxy_address: Option<String>,
    #[serde(skip)]
    proxy_handlers: Option<Vec<RequestHandler>>,
}

impl Config {
    pub const fn new() -> Self {
        Config {
            proxy_address: None,
            proxy_handlers: None,
        }
    }

    pub fn add_proxy_handler(&mut self, handler: RequestHandler) {
        if self.proxy_handlers.is_none() {
            self.proxy_handlers = Some(Vec::new())
        }
        self.proxy_handlers.as_mut().unwrap().push(handler);
    }

    async fn handle_request(&self, req: &mut Request, resp: &mut Response<'_>) {
        for handle_request in self
            .proxy_handlers
            .iter()
            .flat_map(|x| x)
            .map(|x| &x.0)
            .flat_map(|x| x)
        {
            if let Some(fut) = handle_request(req, resp) {
                fut.await
            }
        }
        let url = http::Uri::builder()
            .scheme("https")
            .authority(self.proxy_address.as_ref().unwrap().deref())
            .path_and_query(req.uri.deref())
            .build()
            .unwrap();
        let mut request_builder = http::Request::builder().method(req.method.deref()).uri(url);
        req.headers.insert(
            "Host",
            self.proxy_address
                .as_ref()
                .unwrap()
                .deref()
                .parse()
                .unwrap(),
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
    fn load(config_yaml: &Value, configs: &mut Vec<Option<Plugin>>) -> Option<Arc<Self>>
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
