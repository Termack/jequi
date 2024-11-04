mod content_type;
use derivative::Derivative;
use futures::future::FutureExt;
use jequi::{JequiConfig, Plugin, PostRequestHandler, Request, RequestHandler, Response};
use serde::{de, Deserialize};
use serde_yaml::Value;

use std::{
    any::Any,
    fmt,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
};

pub fn load_plugin(config_yaml: &Value, configs: &mut Vec<Option<Plugin>>) -> Option<Plugin> {
    let config = Config::load(config_yaml, configs)?;
    Some(Plugin {
        config: config.clone(),
        request_handler: RequestHandler(Some(Arc::new(
            move |req: &mut Request, resp: &mut Response| {
                let config = config.clone(); //TODO: figure out some way to avoid this clone
                config.handle_request(req, resp).boxed()
            },
        ))),
    })
}

#[derive(PartialEq, Clone, Debug)]
pub enum PathKind {
    Dir(PathBuf),
    File(PathBuf),
}

impl<'de> Deserialize<'de> for PathKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct PathKindVisitor;

        impl<'de> de::Visitor<'de> for PathKindVisitor {
            type Value = PathKind;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("PathKind")
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_str(&v)
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let path = PathBuf::from(v);
                if !path.exists() {
                    return Err(E::custom(format!("path doesn't exist: {}", path.display())));
                }

                match path.is_dir() {
                    true => Ok(PathKind::Dir(path)),
                    false => Ok(PathKind::File(path)),
                }
            }
        }

        deserializer.deserialize_string(PathKindVisitor {})
    }
}

impl Default for PathKind {
    fn default() -> Self {
        Self::Dir(PathBuf::new())
    }
}

#[derive(Deserialize, Clone, Debug, PartialEq, Derivative)]
#[derivative(Default)]
#[serde(default)]
pub struct Config {
    pub static_files_path: Option<PathKind>,
    #[derivative(Default(value = "true"))]
    pub infer_content_type: bool,
    pub not_found_file_path: Option<PathBuf>,
    config_path: Option<String>,
}

impl Config {
    async fn handle_request(
        self: Arc<Self>,
        req: &Request,
        resp: &mut Response,
    ) -> PostRequestHandler {
        let final_path = &mut PathBuf::new();
        match self.static_files_path.as_ref().unwrap() {
            PathKind::File(file_path) => final_path.push(file_path),
            PathKind::Dir(dir_path) => {
                let mut path = req.uri.path();
                if let Some(path_config) = self.config_path.as_deref() {
                    path = path.strip_prefix(path_config).unwrap_or(path);
                }
                path = path.trim_start_matches('/');

                let mut file_path = PathBuf::new();
                for p in Path::new(path) {
                    if p == ".." {
                        file_path.pop();
                    } else {
                        file_path.push(p)
                    }
                }

                if file_path == PathBuf::new() {
                    file_path.push("index.html")
                }

                final_path.push(dir_path);
                final_path.push(file_path);
            }
        };

        match std::fs::read(&final_path) {
            Ok(content) => resp.write_body(&content).unwrap(),
            Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                resp.status = 403;
                return PostRequestHandler::Exit;
            }
            Err(_) => {
                resp.status = 404;
                if let Some(not_found_path) = self.not_found_file_path.as_ref() {
                    if let Ok(content) = std::fs::read(not_found_path) {
                        resp.write_body(&content).unwrap();
                    }
                }
                return PostRequestHandler::Exit;
            }
        };

        resp.status = 200;

        if self.infer_content_type {
            if let Some(content_type) = content_type::get_content_type_by_path(final_path) {
                resp.set_header("Content-Type", content_type);
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
        if conf == Config::default() || conf.static_files_path.is_none() {
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

#[cfg(test)]
mod tests {

    static TEST_PATH: &str = "test/";

    use std::{
        fs::{self, File},
        io::Cursor,
        os::unix::prelude::PermissionsExt,
        path::Path,
        sync::Arc,
    };

    use http::HeaderValue;
    use jequi::{http1::Http1Conn, AsyncRWSendBuf, RawStream, Uri};
    use tokio::io::{AsyncRead, AsyncWrite};

    use crate::{Config, PathKind};

    async fn test_handle_request<T: AsyncRWSendBuf>(
        conf: Arc<Config>,
        http: &mut Http1Conn<T>,
        uri: &str,
        expected_status: usize,
        expected_resp: &[u8],
    ) {
        http.request.uri = Uri::from(uri.to_string());
        http.response.body_buffer.truncate(0);

        conf.handle_request(&http.request, &mut http.response).await;

        assert_eq!(
            http.response.status, expected_status,
            "status error for {}",
            uri
        );
        assert_eq!(
            &http.response.body_buffer[..],
            expected_resp,
            "response error for {}",
            uri
        );
    }

    #[tokio::test]
    async fn handle_static_files_test() {
        let mut http = Http1Conn::new(RawStream::Normal(Cursor::new(vec![])));

        let conf = &mut Arc::new(Config {
            static_files_path: Some(PathKind::Dir(TEST_PATH.into())),
            not_found_file_path: Some(format!("{}notfound.html", TEST_PATH).into()),
            ..Default::default()
        });

        // Normal test
        test_handle_request(conf.clone(), &mut http, "/file", 200, b"hello").await;

        // Content type test
        test_handle_request(
            conf.clone(),
            &mut http,
            "/aa.js",
            200,
            b"console.log(\"a\")\n",
        )
        .await;

        assert_eq!(
            http.response.get_header("Content-Type"),
            Some(&HeaderValue::from_static("text/javascript"))
        );

        // lfi test
        test_handle_request(conf.clone(), &mut http, "/file/./../../file", 200, b"hello").await;

        // Forbidden test
        let path = format!("{}noperm", TEST_PATH);
        let path = Path::new(&path);

        if !path.exists() {
            File::create(path).unwrap();
        }

        fs::set_permissions(path, fs::Permissions::from_mode(0o000)).unwrap();

        test_handle_request(conf.clone(), &mut http, "/noperm", 403, b"").await;

        // Notfound test
        test_handle_request(conf.clone(), &mut http, "/notfound", 404, b"not found\n").await;

        // Uri config test
        Arc::get_mut(conf).unwrap().config_path = Some("/uri".to_string());

        test_handle_request(conf.clone(), &mut http, "/uri/file", 200, b"hello").await;

        // File as path test
        Arc::get_mut(conf).unwrap().static_files_path =
            Some(PathKind::File(format!("{}file", TEST_PATH).into()));
        Arc::get_mut(conf).unwrap().config_path = None;

        test_handle_request(conf.clone(), &mut http, "/blablabla", 200, b"hello").await;
    }
}
