use serde_yaml;

use crate::Config;

impl Clone for Config {
    fn clone(&self) -> Config {
        Config {
            ip: self.ip.clone(),
            port: self.port,
            static_files_path: self.static_files_path.clone(),
            tls_active: self.tls_active,
            go_handler_path: self.go_handler_path.clone(),
            go_library_path: self.go_library_path.clone(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ip: String::from("127.0.0.1"),
            port: 7878,
            static_files_path: None,
            tls_active: false,
            go_handler_path: None,
            go_library_path: None
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

#[cfg(test)]
mod tests {
    use crate::Config;

    static CONF_TEST_PATH: &str = "test/test.conf";

    #[test]
    fn load_config_test() {
        let config = Config::load_config(CONF_TEST_PATH);

        let mut test_config = Config::default();
        test_config.tls_active = true;
        test_config.static_files_path = Some("./".to_string());

        assert_eq!(
            config,
            test_config
        )
    }
}