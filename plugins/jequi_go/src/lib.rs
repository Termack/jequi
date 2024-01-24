#![feature(get_mut_unchecked)]
use jequi::{JequiConfig, Plugin, Request, RequestHandler, Response};
use libloading::{self, Library};
use plugins::get_plugin;
use serde::Deserialize;
use serde_yaml::Value;
use std::any::Any;
use std::sync::Arc;

pub fn load_plugin(config_yaml: &Value, configs: &mut Vec<Option<Plugin>>) -> Option<Plugin> {
    let config = Config::load(config_yaml, configs)?;
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

#[derive(Default, Debug)]
struct Lib(Option<Library>);

impl PartialEq for Lib {
    fn eq(&self, other: &Self) -> bool {
        self.0.is_none() && other.0.is_none()
    }
}

#[derive(Deserialize, Default, Debug, PartialEq)]
pub struct Config {
    pub go_library_path: Option<String>,
    #[serde(skip)]
    lib: Lib,
}

impl Config {
    pub const fn new() -> Self {
        Config {
            go_library_path: None,
            lib: Lib(None),
        }
    }

    pub fn handle_request(&self, req: &mut Request, resp: &mut Response) {
        let lib = self.lib.0.as_ref().unwrap();
        unsafe {
            let go_handle_response: libloading::Symbol<
                unsafe extern "C" fn(req: *mut Request, resp: *mut Response),
            > = lib.get(b"HandleRequest\0").unwrap();
            go_handle_response(req, resp);
        }
    }

    pub fn handle_request_proxy(&self, req: &mut Request, resp: &mut Response) {
        println!("heyeyeyeyeyeyeyey");
    }
}

impl JequiConfig for Config {
    fn load(config_yaml: &Value, configs: &mut Vec<Option<Plugin>>) -> Option<Arc<Self>>
    where
        Self: Sized,
    {
        let mut conf: Config = Deserialize::deserialize(config_yaml).unwrap();
        if conf == Config::default() {
            return None;
        }

        unsafe {
            let lib = Library::new(conf.go_library_path.as_ref().unwrap()).unwrap();
            conf.lib = Lib(Some(lib));
        }

        let proxy_conf = get_plugin!(configs, jequi_proxy, mut Option);

        let conf = Arc::new(conf);
        if let Some(proxy_conf) = proxy_conf {
            let conf2 = conf.clone();

            proxy_conf.add_proxy_handler(RequestHandler(Some(Arc::new(
                move |req: &mut Request, resp: &mut Response<'_>| {
                    conf2.handle_request_proxy(req, resp);
                    None
                },
            ))));
        }

        Some(conf)
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
    static GO_LIB_FILE: &str = "jequi_go.so";

    use std::{io::Cursor, process::Command};

    use jequi::{HttpConn, JequiConfig, RawStream};
    use serde_yaml::{Mapping, Value};

    use crate::Config;

    #[tokio::test]
    async fn handle_go_request() {
        let mut buf = [0; 35];
        let mut http = HttpConn::new(
            RawStream::Normal(Cursor::new(vec![])),
            &mut [0; 0],
            &mut buf,
        )
        .await;

        let output = Command::new("go")
            .args([
                "build",
                "-C",
                TEST_PATH,
                "-o",
                GO_LIB_FILE,
                "-buildmode=c-shared",
            ])
            .output()
            .expect("failed to build go code");

        assert!(
            output.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout[..]),
            String::from_utf8_lossy(&output.stderr[..])
        );

        let go_library_path = format!("{}/{}", TEST_PATH, GO_LIB_FILE);
        let mut yaml_config = Mapping::new();
        yaml_config.insert("go_library_path".into(), go_library_path.clone().into());

        let conf = Config::load(&Value::Mapping(yaml_config), &mut Vec::new()).unwrap();

        http.request.uri = "/file".to_string();

        conf.handle_request(&mut http.request, &mut http.response);

        assert_eq!(http.response.status, 200);
        assert_eq!(
            &http.response.body_buffer[..http.response.body_length],
            b"hello"
        );
        assert_eq!(http.response.get_header("test").unwrap(), &http.request.uri);
    }
}
