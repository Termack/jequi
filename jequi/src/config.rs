use std::{any::Any, collections::HashMap, sync::Arc};

use serde_yaml::{self, from_reader, Value};
use serde_yaml::from_value;

use crate::{Config, ConfigMap, JequiConfig, MainConf, RequestHandler};

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
    use crate::{Config, MainConf};

    static CONF_TEST_PATH: &str = "test/test.conf";

    #[test]
    fn load_config_test() {
        let mut main_config = MainConf::default();
        main_config.load_config(CONF_TEST_PATH);

        let conf = main_config.config_map.get("main").unwrap().to_owned();
        let conf = conf.as_any().downcast_ref::<Config>().unwrap().clone();

        let mut test_config = Config::default();
        test_config.tls_active = true;
        test_config.ip = "1.2.3.4".to_owned();

        assert_eq!(conf, test_config)
    }
}
