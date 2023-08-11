use serde_yaml;

use crate::Config;

impl Config {
    fn new() -> Config {
        Config {
            ip: String::from("127.0.0.1"),
            port: 7878,
            static_files_path: Some(String::from("./")),
        }
    }

    pub fn load_config(filename: &str) -> Config {
        let file_reader: std::fs::File = match std::fs::File::open(filename) {
            Ok(file_reader) => file_reader,
            Err(_) => return Config::new(),
        };
        let config: Config = serde_yaml::from_reader(file_reader)
            .expect(&format!("Failed to parse config for `{}`", filename));
        config
    }
}
