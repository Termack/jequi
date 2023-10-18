use jequi::{JequiConfig, Plugin, Request, RequestHandler, Response};
use libc;
use libloading::{self, Library, Symbol};
use serde::Deserialize;
use serde_yaml::Value;
use std::any::Any;
use std::process::Command;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, path::Path};
use std::{fs, path, process};

pub fn load_plugin(config: &Value) -> Option<Plugin> {
    let config = Arc::new(Config::load(config)?);
    Some(Plugin {
        config: config.clone(),
        request_handler: Some(config.clone()),
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
    pub go_handler_path: Option<String>,
    library_path: Option<String>,
    #[serde(skip)]
    lib: Lib,
}

impl Config {
    pub const fn new() -> Self {
        Config {
            go_handler_path: None,
            library_path: None,
            lib: Lib(None),
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

        let mut lib_path = match env::var("LIB_DIR") {
            Ok(dir) => dir,
            Err(_) => "target/debug".to_string(),
        };

        lib_path = fs::canonicalize(format!("{}/jequi_go.so", lib_path))
            .unwrap()
            .into_os_string()
            .into_string()
            .unwrap();

        let milis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let temp_file_path = format!("{}.{}", &lib_path, milis);
        fs::copy(&lib_path, &temp_file_path).unwrap();

        fs::create_dir_all("/tmp/jequi_go_build").unwrap();
        let mut binding = Command::new("go");
        let command = binding
            .arg("build")
            .args([
                "-C",
                &env::current_dir()
                    .unwrap()
                    .into_os_string()
                    .into_string()
                    .unwrap(),
            ])
            .args(["-o", &temp_file_path])
            .arg("-buildmode=c-shared")
            .arg("-work")
            .args([
                "-ldflags",
                &format!("-o {} -v -tmpdir /tmp/jequi_go_build", &temp_file_path),
            ]);
        println!("{:?}", command);
        let out = command.output().unwrap();
        println!("{}", String::from_utf8_lossy(&out.stderr));
        for mut line in String::from_utf8_lossy(&out.stderr).lines() {
            if line.starts_with("host link:") {
                line = line.trim_start_matches("host link: \"");
                line = line.trim_end_matches("\"");
                let args: Vec<&str> = line
                    .split("\" \"")
                    .filter(|&x| x != "-Wl,-z,nodelete")
                    .collect();
                println!("{:?}", args);
                let out = Command::new(args[0]).args(&args[1..]).output().unwrap();
                println!("{}", String::from_utf8_lossy(&out.stdout));
                println!("{}", String::from_utf8_lossy(&out.stderr));
            }
        }
        // unsafe {
        //     Library::new(&lib_path).unwrap();
        // }
        // let a = lib_path.clone();
        // tokio::spawn( async move {
        //     unsafe {
        //     let lib = libloading::os::unix::Library::open(Some(a), libc::RTLD_NOW).unwrap();
        //     let close: libloading::os::unix::Symbol<
        //         unsafe extern "C" fn(),
        //     > = lib.get(b"Close\0").unwrap();
        //     close();
        //     println!("{:?}",lib.close());
        // }});
        conf.library_path = Some(lib_path);
        let proc_path = format!("/proc/{}/task", process::id().to_string());
        println!("lalal");
        for entry in fs::read_dir(&proc_path).unwrap().enumerate() {
            println!("{:?}", entry)
        }
        unsafe {
            conf.lib = Lib(Some(Library::new(&temp_file_path).unwrap()));
        }
        fs::remove_file(temp_file_path).unwrap();
        println!("lalal");
        for entry in fs::read_dir(&proc_path).unwrap().enumerate() {
            println!("{:?}", entry)
        }
        Some(conf)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl RequestHandler for Config {
    fn handle_request(&self, req: &mut Request, resp: &mut Response) {
        let proc_path = format!("/proc/{}/task", process::id().to_string());
        println!("lalal");
        for entry in fs::read_dir(&proc_path).unwrap().enumerate() {
            println!("{:?}", entry)
        }
        unsafe {
            let lib = self.lib.0.as_ref().unwrap();
            let go_handle_response: Symbol<
                unsafe extern "C" fn(req: *mut Request, resp: *mut Response),
            > = lib.get(b"HandleRequest\0").unwrap();
            go_handle_response(req, resp);
            let close: Symbol<unsafe extern "C" fn()> = lib.get(b"Close\0").unwrap();
            close();
        }
    }
}
