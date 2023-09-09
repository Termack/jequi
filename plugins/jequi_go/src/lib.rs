use jequi::{JequiConfig, Request, RequestHandler, Response, ConfigMap};
use libloading::{self, Library};
use std::any::Any;
use serde::Deserialize;
use std::fs;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use serde_yaml::from_value;
use std::{env, path::Path};

pub fn name() -> String {
    "jequi_go".to_owned()
}

#[derive(Deserialize,Clone, Default)]
pub struct Config {
    pub handler_path: Option<String>,
    library_path: Option<String>,
}

impl Config {
    pub const fn new() -> Self {
        Config { handler_path: None, library_path: None }
    }
}

impl JequiConfig for Config {
    fn load(config: &mut ConfigMap, handlers: &mut Vec<Arc<dyn RequestHandler>>) -> Option<Arc<Self>> where Self: Sized {
        let mut conf: Config = from_value(config.remove(&name())?).unwrap();

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
        let new_conf = Arc::new(conf);
        handlers.push(new_conf.clone());
        Some(new_conf)
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