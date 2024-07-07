use std::io::{Error, ErrorKind, Result};
use std::sync::Arc;

use chrono::Utc;
use http::{HeaderMap, HeaderValue};

use crate::body::GetBody;
use crate::{body::RequestBody, Request};
use crate::{ConfigMap, PostRequestHandler, Response, Uri};

impl From<String> for Uri {
    fn from(item: String) -> Self {
        Self(item)
    }
}

impl Uri {
    pub fn raw(&self) -> &str {
        self.0.as_str()
    }

    pub fn path(&self) -> &str {
        self.0.splitn(2, '?').next().unwrap()
    }

    pub fn query_string(&self) -> Option<&str> {
        let mut it = self.0.splitn(2, '?');
        it.next();
        it.next()
    }
}

impl Request {
    pub fn new() -> Request {
        Request {
            method: String::new(),
            uri: Uri::from(String::new()),
            headers: HeaderMap::new(),
            host: None,
            body: Arc::new(RequestBody::default()),
        }
    }

    pub fn get_content_length(&self) -> Result<usize> {
        self.get_header("Content-Length")
            .ok_or(Error::new(ErrorKind::NotFound, "No content length"))?
            .to_str()
            .map_err(|err| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("Cant convert content length to int: {}", err),
                )
            })?
            .parse()
            .map_err(|err| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("Cant convert content length to int: {}", err),
                )
            })
    }

    pub async fn handle_request(
        &mut self,
        response: &mut Response,
        config_map: Arc<ConfigMap>,
    ) -> PostRequestHandler {
        response.set_header("server", "jequi");
        response.set_header(
            "date",
            &Utc::now().format("%a, %e %b %Y %T GMT").to_string(),
        );

        let config = config_map.get_config_for_request(self.host.as_deref(), Some(self.uri.path()));

        for handle_plugin in config.iter().map(|x| &x.request_handler.0).flat_map(|x| x) {
            match handle_plugin(self, response).await {
                PostRequestHandler::Continue => (),
                post_handler => return post_handler,
            }
        }

        if response.status == 0 {
            response.status = 200;
        }

        PostRequestHandler::Continue
    }

    pub fn get_header(&self, header: &str) -> Option<&HeaderValue> {
        self.headers.get(header.to_lowercase().trim())
    }

    pub fn get_body(&self) -> GetBody {
        RequestBody::get_body(self.body.clone())
    }
}

impl Default for Request {
    fn default() -> Self {
        Self::new()
    }
}
