use std::io::{Error, ErrorKind, Result};
use std::sync::Arc;

use chrono::Utc;
use http::{HeaderMap, HeaderValue};

use crate::body::GetBody;
use crate::{body::RequestBody, Request};
use crate::{ConfigMap, Response};

impl Request {
    pub fn new() -> Request {
        Request {
            method: String::new(),
            uri: String::new(),
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

    pub async fn handle_request(&mut self, response: &mut Response, config_map: Arc<ConfigMap>) {
        response.set_header("server", "jequi");
        response.set_header(
            "date",
            &Utc::now().format("%a, %e %b %Y %T GMT").to_string(),
        );

        let config = config_map.get_config_for_request(self.host.as_deref(), Some(&self.uri));

        for handle_plugin in config.iter().map(|x| &x.request_handler.0).flat_map(|x| x) {
            if let Some(fut) = handle_plugin(self, response) {
                fut.await
            }
        }

        if response.status == 0 {
            response.status = 200;
        }
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
