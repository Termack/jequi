use jequi::{JequiConfig, Plugin, Request, RequestHandler, Response};
use libloading::{self, Library};
use serde::Deserialize;
use serde_yaml::Value;
use std::any::Any;
use std::fs;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, path::Path};

pub fn name() -> String {
    "jequi_go".to_owned()
}

pub fn load_plugin(config: &Value) -> Option<Plugin> {
    let config = Arc::new(Config::load(config)?);
    Some(Plugin {
        config: config.clone(),
        request_handler: Some(config.clone()),
    })
}

#[derive(Deserialize, Clone, Default, Debug, PartialEq)]
pub struct Config {
    pub go_handler_path: Option<String>,
    library_path: Option<String>,
}

impl Config {
    pub const fn new() -> Self {
        Config {
            go_handler_path: None,
            library_path: None,
        }
    }
}

impl JequiConfig for Config {
    fn load(config: &Value) -> Option<Self>
    where
        Self: Sized,
    {
        let mut conf: Config = Deserialize::deserialize(config).unwrap();
        if conf == Config::default() {
            return None;
        }

        if let Some(lib_path) = conf.library_path.as_ref() {
            if Path::new(&lib_path).exists() {
                fs::remove_file(lib_path).unwrap();
            }
        }

        let mut original_lib_path = match env::var("LIB_DIR") {
            Ok(dir) => dir,
            Err(_) => "target/debug".to_string(),
        };

        original_lib_path = format!("{}/jequi_go.so", original_lib_path);
        let milis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let new_file_path = format!("/tmp/jequi_go.{}.so", milis);
        fs::copy(original_lib_path, &new_file_path).unwrap();
        unsafe {
            Library::new(&new_file_path).unwrap();
        }
        conf.library_path = Some(new_file_path);
        Some(conf)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl RequestHandler for Config {
    fn handle_request(&self, req: &mut Request, resp: &mut Response) {
        let path = self.library_path.as_ref().unwrap();
        unsafe {
            let lib = libloading::Library::new(path).unwrap();
            let go_handle_response: libloading::Symbol<
                unsafe extern "C" fn(req: *mut Request, resp: *mut Response),
            > = lib.get(b"HandleRequest\0").unwrap();
            go_handle_response(req, resp);
        }
    }
}
