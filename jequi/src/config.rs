use std::{any::Any, sync::Arc};

use serde::Deserialize;
use serde_yaml::from_reader;

use crate::{Config, JequiConfig, Value, ConfigMapParser, Plugin};

pub fn load_plugin(config: &Value) -> Option<Plugin> {
    let config = Arc::new(Config::load(config)?);
    Some(Plugin {
        config: config.clone(),
        request_handler: None,
    })
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ip: String::from("127.0.0.1"),
            port: 7878,
            tls_active: false,
        }
    }
}

impl JequiConfig for Config {
    fn load(config: &Value) -> Option<Self>
    where
        Self: Sized,
    {
        let conf: Config = Deserialize::deserialize(config).unwrap();
        Some(conf)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ConfigMapParser {
    pub fn load_config(filename: &str) -> ConfigMapParser {
        let file_reader = std::fs::File::open(filename).unwrap();
        let config_map: ConfigMapParser =
            from_reader(file_reader).expect(&format!("Failed to parse config for `{}`", filename));

        config_map
    }
}

#[cfg(test)]
mod tests {
    use crate::{ConfigMapParser, Config, JequiConfig};

    static CONF_TEST_PATH: &str = "test/test.conf";

    #[test]
    fn load_config_test() {
        let mut main_conf = ConfigMapParser::load_config(CONF_TEST_PATH);

        let conf = Config::load(&mut main_conf.config).unwrap();

        let mut test_config = Config::default();
        test_config.tls_active = true;
        test_config.ip = "1.2.3.4".to_owned();

        assert_eq!(conf, test_config)
    }
}
