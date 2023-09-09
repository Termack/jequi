use std::{any::Any, sync::Arc};

use serde_yaml::{from_reader, from_value};

use crate::{Config, ConfigMap, JequiConfig, RequestHandler};

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
    fn load(config: &mut ConfigMap, _handlers: &mut Vec<Arc<dyn RequestHandler>>) -> Option<Arc<Self>>
    where
        Self: Sized,
    {
        let conf: Config = from_value(config.remove("main")?).unwrap();
        let new_conf = Arc::new(conf);
        Some(new_conf)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn load_config(filename: &str) -> ConfigMap {
    let file_reader = std::fs::File::open(filename).unwrap();
    let config_map: ConfigMap =
        from_reader(file_reader).expect(&format!("Failed to parse config for `{}`", filename));

    config_map
}

#[cfg(test)]
mod tests {
    use crate::{Config, config::load_config, JequiConfig};

    static CONF_TEST_PATH: &str = "test/test.conf";

    #[test]
    fn load_config_test() {
        let mut main_conf = load_config(CONF_TEST_PATH);

        let conf = Config::load(&mut main_conf, &mut Vec::new()).unwrap();

        let mut test_config = Config::default();
        test_config.tls_active = true;
        test_config.ip = "1.2.3.4".to_owned();

        assert_eq!(*conf, test_config)
    }
}
