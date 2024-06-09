use core::panic;
use std::{
    any::Any,
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::Deserialize;
use serde_yaml::from_reader;

use crate::{Config, ConfigMap, ConfigMapParser, HostConfig, JequiConfig, Plugin, Value};

impl Default for Config {
    fn default() -> Self {
        Self {
            ip: String::from("127.0.0.1"),
            port: 7878,
            tls_active: false,
            http2: false,
            chunk_size: 1024 * 64,
            ssl_certificate: None,
            ssl_key: None,
        }
    }
}

impl JequiConfig for Config {
    fn load(config_yaml: &Value, _configs: &mut Vec<Option<Plugin>>) -> Option<Arc<Self>>
    where
        Self: Sized,
    {
        let conf: Config = Deserialize::deserialize(config_yaml).unwrap();
        Some(Arc::new(conf))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn merge_config_and_load_plugins(
    mut _config_parser: &mut Value,
    config_to_merge: &mut Value,
    key_to_add: &str,
    value_to_add: String,
    load_plugins: fn(&Value) -> Vec<Plugin>,
) -> Vec<Plugin> {
    // TODO: maybe merging doesn't make sense, i need to think about it
    // merge_yaml(&mut config_parser, config_to_merge);
    let config_parser = config_to_merge;
    if let Value::Mapping(ref mut config_parser) = config_parser {
        config_parser.insert(format!("config_{}", key_to_add).into(), value_to_add.into());
    }
    load_plugins(config_parser)
}

impl ConfigMap {
    pub fn load(path: &str, load_plugins: fn(&Value) -> Vec<Plugin>) -> ConfigMap {
        let mut main_conf = ConfigMap::default();
        let main_conf_parser = ConfigMapParser::load_config(path);

        let config_parser = main_conf_parser.config.clone();
        for (host, mut host_config_parser) in main_conf_parser.host.into_iter().flatten() {
            let mut config_parser = config_parser.clone();
            let plugin_list = merge_config_and_load_plugins(
                &mut config_parser,
                &mut host_config_parser.config,
                "host",
                host.clone(),
                load_plugins,
            );
            let mut path_config: Option<HashMap<PathBuf, Vec<Plugin>>> = None;
            for (path, mut path_config_parser) in host_config_parser.path.into_iter().flatten() {
                let mut config_parser = config_parser.clone();
                let plugin_list = merge_config_and_load_plugins(
                    &mut config_parser,
                    &mut path_config_parser,
                    "path",
                    path.to_string_lossy().to_string(),
                    load_plugins,
                );
                path_config
                    .get_or_insert_default()
                    .insert(path, plugin_list);
            }
            main_conf.host.get_or_insert_default().insert(
                host,
                HostConfig {
                    path: path_config,
                    config: plugin_list,
                },
            );
        }

        for (path, mut path_config_parser) in main_conf_parser.path.into_iter().flatten() {
            let mut config_parser = config_parser.clone();
            let plugin_list = merge_config_and_load_plugins(
                &mut config_parser,
                &mut path_config_parser,
                "path",
                path.to_string_lossy().to_string(),
                load_plugins,
            );
            main_conf
                .path
                .get_or_insert_default()
                .insert(path, plugin_list);
        }

        main_conf.config = load_plugins(&config_parser);
        main_conf
    }

    pub fn get_config_for_request(&self, host: Option<&str>, path: Option<&str>) -> &Vec<Plugin> {
        let mut config = &self.config;
        let mut path_map = &self.path;
        if let Some(host_map) = &self.host
            && let Some(host) = host
            && let Some(host_config) = host_map.get(host.split(':').next().unwrap())
        {
            config = &host_config.config;
            path_map = &host_config.path;
        }

        let path = match path {
            Some(path) => path,
            None => return config,
        };

        let path_map = match path_map {
            Some(path_map) => {
                if path_map.is_empty() {
                    return config;
                }
                path_map
            }
            None => return config,
        };

        let mut path = PathBuf::from(path);

        if let Some(config) = path_map.get(&path) {
            return config;
        }

        loop {
            if let Some(config) = path_map.get(&path) {
                return config;
            }

            if !path.pop() {
                return config;
            }
        }
    }
}

impl ConfigMapParser {
    pub fn load_config(filename: &str) -> ConfigMapParser {
        let file_reader = std::fs::File::open(filename).unwrap();
        let config_map: ConfigMapParser = from_reader(file_reader)
            .unwrap_or_else(|_| panic!("Failed to parse config for `{}`", filename));

        config_map
    }
}

#[cfg(test)]
mod tests {
    use std::{path::Path, sync::Arc};

    use crate::{load_plugin, Config, ConfigMap, ConfigMapParser, JequiConfig, Plugin};

    static CONF_TEST_PATH: &str = "test/test.conf";

    #[test]
    fn load_config_test() {
        let main_conf = ConfigMapParser::load_config(CONF_TEST_PATH);

        let conf = Config::load(&main_conf.config, &mut Vec::new()).unwrap();

        let test_config = Arc::new(Config {
            ip: "1.1.1.1".to_owned(),
            tls_active: true,
            ..Default::default()
        });

        assert_eq!(conf, test_config)
    }

    #[test]
    fn get_config_for_request_test() {
        let config_map = ConfigMap::load(CONF_TEST_PATH, |val| {
            vec![load_plugin(val, &mut Vec::new()).unwrap()]
        });

        let get_config: fn(&Vec<Plugin>) -> &Config = |conf| {
            conf.get(0)
                .unwrap()
                .config
                .as_any()
                .downcast_ref::<Config>()
                .unwrap()
        };

        assert_eq!(get_config(&config_map.config).ip, "1.1.1.1");
        let host_jequi_com = config_map.host.as_ref().unwrap().get("jequi.com").unwrap();
        assert_eq!(get_config(&host_jequi_com.config).ip, "1.1.2.1");
        assert_eq!(
            get_config(
                host_jequi_com
                    .path
                    .as_ref()
                    .unwrap()
                    .get(Path::new("/app"))
                    .unwrap()
            )
            .ip,
            "1.1.2.2"
        );
        assert_eq!(
            get_config(
                host_jequi_com
                    .path
                    .as_ref()
                    .unwrap()
                    .get(Path::new("/api"))
                    .unwrap()
            )
            .ip,
            "1.1.2.3"
        );
        assert_eq!(
            get_config(
                &config_map
                    .host
                    .as_ref()
                    .unwrap()
                    .get("www.jequi.com")
                    .unwrap()
                    .config
            )
            .ip,
            "1.1.3.1"
        );
        assert_eq!(
            get_config(
                config_map
                    .path
                    .as_ref()
                    .unwrap()
                    .get(Path::new("/app"))
                    .unwrap()
            )
            .ip,
            "1.2.1.1"
        );
        assert_eq!(
            get_config(
                config_map
                    .path
                    .as_ref()
                    .unwrap()
                    .get(Path::new("/test"))
                    .unwrap()
            )
            .ip,
            "1.2.1.2"
        );

        assert_eq!(
            get_config(config_map.get_config_for_request(None, Some("/"))).ip,
            "1.1.1.1"
        );
        assert_eq!(
            get_config(config_map.get_config_for_request(Some("jequi.com"), Some("/test"))).ip,
            "1.1.2.1"
        );
        assert_eq!(
            get_config(config_map.get_config_for_request(Some("jequi.com"), Some("/app/hello"))).ip,
            "1.1.2.2"
        );
        assert_eq!(
            get_config(config_map.get_config_for_request(Some("jequi.com"), Some("/api/"))).ip,
            "1.1.2.3"
        );
        assert_eq!(
            get_config(config_map.get_config_for_request(Some("www.jequi.com"), Some("/test"))).ip,
            "1.1.3.1"
        );
        assert_eq!(
            get_config(config_map.get_config_for_request(None, Some("/app/hey"))).ip,
            "1.2.1.1"
        );
        assert_eq!(
            get_config(config_map.get_config_for_request(None, Some("/test"))).ip,
            "1.2.1.2"
        );
    }
}
