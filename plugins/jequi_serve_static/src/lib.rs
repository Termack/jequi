use jequi::{JequiConfig, Plugin, Request, RequestHandler, Response};
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
                config.handle_request(req, resp);
                None
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

#[derive(Deserialize, Clone, Default, Debug, PartialEq)]
pub struct Config {
    pub static_files_path: Option<PathKind>,
    uri: Option<String>,
}

impl Config {
    pub const fn new() -> Self {
        Config {
            static_files_path: None,
            uri: None,
        }
    }

    fn handle_request(&self, req: &Request, resp: &mut Response) {
        let final_path = &mut PathBuf::new();
        match self.static_files_path.as_ref().unwrap() {
            PathKind::File(file_path) => final_path.push(file_path),
            PathKind::Dir(path) => {
                let mut uri = req.uri.as_str();
                if let Some(uri_config) = self.uri.as_deref() {
                    uri = uri.strip_prefix(uri_config).unwrap_or(uri);
                }
                uri = uri.trim_start_matches('/');

                let mut file_path = PathBuf::new();
                for p in Path::new(uri) {
                    if p == ".." {
                        file_path.pop();
                    } else {
                        file_path.push(p)
                    }
                }

                if file_path == PathBuf::new() {
                    file_path.push("index.html")
                }

                final_path.push(path);
                final_path.push(file_path);
            }
        };

        match std::fs::read(final_path) {
            Ok(content) => resp.write_body(&content).unwrap(),
            Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                resp.status = 403;
                return;
            }
            Err(_) => {
                resp.status = 404;
                return;
            }
        };

        resp.status = 200;
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
    };

    use jequi::{HttpConn, RawStream};

    use crate::{Config, PathKind};

    #[tokio::test]
    async fn handle_static_files_test() {
        let mut http = HttpConn::new(RawStream::Normal(Cursor::new(vec![])));

        // Normal test
        http.request.uri = "/file".to_string();

        let mut conf = Config {
            static_files_path: Some(PathKind::Dir(TEST_PATH.into())),
            ..Default::default()
        };

        conf.handle_request(&http.request, &mut http.response);

        assert_eq!(http.response.status, 200);
        assert_eq!(&http.response.body_buffer[..], b"hello");

        // lfi test
        http.request.uri = "/file/./../../file".to_string();
        http.response.body_buffer.truncate(0);

        conf.handle_request(&http.request, &mut http.response);

        assert_eq!(http.response.status, 200);
        assert_eq!(&http.response.body_buffer[..], b"hello");

        // Forbidden test
        let path = format!("{}noperm", TEST_PATH);
        let path = Path::new(&path);

        if !path.exists() {
            File::create(path).unwrap();
        }

        fs::set_permissions(path, fs::Permissions::from_mode(0o000)).unwrap();

        http.request.uri = "/noperm".to_string();
        http.response.body_buffer.truncate(0);

        conf.handle_request(&http.request, &mut http.response);

        assert_eq!(http.response.status, 403);
        assert_eq!(&http.response.body_buffer[..], b"");

        // Notfound test
        http.request.uri = "/notfound".to_string();
        http.response.body_buffer.truncate(0);

        conf.handle_request(&http.request, &mut http.response);

        assert_eq!(http.response.status, 404);
        assert_eq!(&http.response.body_buffer[..], b"");

        // Uri config test
        conf.uri = Some("/uri".to_string());

        http.request.uri = "/uri/file".to_string();
        http.response.body_buffer.truncate(0);

        conf.handle_request(&http.request, &mut http.response);

        assert_eq!(http.response.status, 200);
        assert_eq!(&http.response.body_buffer[..], b"hello");

        // File as path test
        conf.static_files_path = Some(PathKind::File(format!("{}file", TEST_PATH).into()));
        conf.uri = None;

        http.request.uri = "/blablabla".to_string();
        http.response.body_buffer.truncate(0);

        conf.handle_request(&http.request, &mut http.response);

        assert_eq!(http.response.status, 200);
        assert_eq!(&http.response.body_buffer[..], b"hello");
    }
}
