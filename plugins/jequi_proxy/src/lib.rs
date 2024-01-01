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

pub fn load_plugin(config: &Value) -> Option<Plugin> {
    let config = Arc::new(Config::load(config)?);
    Some(Plugin {
        config: config.clone(),
        request_handler: RequestHandler(Some(Arc::new(move |req, resp| {
            let config = config.clone(); //TODO: figure out some way to avoid this clone
            Some(async move { config.handle_request(req, resp).await }.boxed())
        }))),
    })
}

#[derive(Deserialize, Default, Debug, PartialEq)]
pub struct Config {
    pub proxy_address: Option<String>,
}

impl Config {
    pub const fn new() -> Self {
        Config {
            proxy_address: None,
        }
    }

    async fn handle_request(&self, req: &mut Request, resp: &mut Response<'_>) {
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
    fn load(config: &Value) -> Option<Self>
    where
        Self: Sized,
    {
        let conf: Config = Deserialize::deserialize(config).unwrap();
        if conf == Config::default() {
            return None;
        }

        Some(conf)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
