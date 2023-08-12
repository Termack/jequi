use serde_yaml;

use crate::Config;

impl Clone for Config {
    fn clone(&self) -> Config {
        Config {
            ip: self.ip.clone(),
            port: self.port,
            static_files_path: self.static_files_path.clone(),
            tls_active: self.tls_active
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ip: String::from("127.0.0.1"),
            port: 7878,
            static_files_path: None,
            tls_active: false
        }
    }
}

impl Config {
    pub fn load_config(filename: &str) -> Config {
        let file_reader: std::fs::File = match std::fs::File::open(filename) {
            Ok(file_reader) => file_reader,
            Err(_) => return Config::default(),
        };
        let config: Config = serde_yaml::from_reader(file_reader)
            .expect(&format!("Failed to parse config for `{}`", filename));
        config
    }
}
