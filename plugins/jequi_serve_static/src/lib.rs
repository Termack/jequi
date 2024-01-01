use jequi::{JequiConfig, Plugin, Request, RequestHandler, Response};
use serde::Deserialize;
use serde_yaml::Value;

use std::{
    any::Any,
    fs::File,
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
    sync::Arc,
};

pub fn load_plugin(config: &Value) -> Option<Plugin> {
    let config = Arc::new(Config::load(config)?);
    Some(Plugin {
        config: config.clone(),
        request_handler: RequestHandler(Some(Arc::new(
            move |req: &mut Request, resp: &mut Response<'_>| {
                config.handle_request(req, resp);
                None
            },
        ))),
    })
}

#[derive(Deserialize, Clone, Default, Debug, PartialEq)]
pub struct Config {
    pub static_files_path: Option<String>,
    uri: Option<String>,
}

impl Config {
    pub const fn new() -> Self {
        Config {
            static_files_path: None,
            uri: None,
        }
    }

    fn handle_request(&self, req: &mut Request, resp: &mut Response) {
        let root = Path::new(self.static_files_path.as_ref().unwrap());

        if !root.exists() {
            resp.status = 404;
            return;
        }

        let mut uri = req.uri.as_str();
        if let Some(uri_config) = self.uri.as_deref() {
            uri = uri.strip_prefix(uri_config).unwrap_or(uri);
        }
        uri = uri.trim_start_matches("/");

        let mut final_path = PathBuf::new();
        for p in Path::new(uri) {
            if p == ".." {
                final_path.pop();
            } else {
                final_path.push(p)
            }
        }

        if final_path == PathBuf::new() {
            final_path.push("index.html")
        }

        final_path = root.join(final_path);

        let mut f = match File::open(final_path) {
            Ok(f) => f,
            Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                resp.status = 403;
                return;
            }
            Err(_) => {
                resp.status = 404;
                return;
            }
        };

        match f.read(&mut resp.body_buffer) {
            Ok(n) => resp.body_length = n,
            Err(_) => {
                resp.status = 404;
                resp.body_length = 0;
                return;
            }
        }

        resp.status = 200;
    }
}

impl JequiConfig for Config {
    fn load(config: &Value) -> Option<Self>
    where
        Self: Sized,
    {
        let conf: Config = Deserialize::deserialize(config).unwrap();
        if conf == Config::default() || conf.static_files_path == None {
            return None;
        }

        Some(conf)
    }
    fn as_any(&self) -> &dyn Any {
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

    use jequi::{HttpConn, RawStream, RequestHandler};

    use crate::Config;

    #[tokio::test]
    async fn handle_static_files_test() {
        let mut buf = [0; 35];
        let mut http = HttpConn::new(
            RawStream::Normal(Cursor::new(vec![])),
            &mut [0; 0],
            &mut buf,
        )
        .await;

        // Normal test
        http.request.uri = "/file".to_string();

        let mut conf = Config::default();
        conf.static_files_path = Some(TEST_PATH.to_owned());

        conf.handle_request(&mut http.request, &mut http.response);

        assert_eq!(http.response.status, 200);
        assert_eq!(
            &http.response.body_buffer[..http.response.body_length],
            b"hello"
        );

        // lfi test
        http.request.uri = "/file/./../../file".to_string();
        http.response.body_length = 0;

        conf.handle_request(&mut http.request, &mut http.response);

        assert_eq!(http.response.status, 200);
        assert_eq!(
            &http.response.body_buffer[..http.response.body_length],
            b"hello"
        );

        // Forbidden test
        let path = format!("{}noperm", TEST_PATH);
        let path = Path::new(&path);

        if !path.exists() {
            File::create(&path).unwrap();
        }

        fs::set_permissions(path, fs::Permissions::from_mode(0o000)).unwrap();

        http.request.uri = "/noperm".to_string();
        http.response.body_length = 0;

        conf.handle_request(&mut http.request, &mut http.response);

        assert_eq!(http.response.status, 403);
        assert_eq!(&http.response.body_buffer[..http.response.body_length], b"");

        // Notfound test
        http.request.uri = "/notfound".to_string();
        http.response.body_length = 0;

        conf.handle_request(&mut http.request, &mut http.response);

        assert_eq!(http.response.status, 404);
        assert_eq!(&http.response.body_buffer[..http.response.body_length], b"");

        // Uri config test
        conf.uri = Some("/uri".to_string());

        http.request.uri = "/uri/file".to_string();
        http.response.body_length = 0;

        conf.handle_request(&mut http.request, &mut http.response);

        assert_eq!(http.response.status, 200);
        assert_eq!(
            &http.response.body_buffer[..http.response.body_length],
            b"hello"
        );
    }
}
