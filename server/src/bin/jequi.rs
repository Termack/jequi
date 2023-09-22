#![feature(let_chains)]
#![feature(option_get_or_insert_default)]
use chrono::Utc;
use jequi::{Config, ConfigMap, ConfigMapParser, HttpConn, Plugin, RawStream};
use plugins;
use std::collections::HashMap;
use std::process;
use std::{fs, io::ErrorKind, sync::Arc};
use tokio::{
    net::{TcpListener, TcpStream},
    signal::unix::{signal, SignalKind},
    spawn,
    sync::RwLock,
};

async fn handle_connection(stream: TcpStream, config_map: Arc<ConfigMap>) {
    let mut read_buffer = [0; 1024];
    let mut body_buffer = [0; 1024];
    let mut http: HttpConn<'_, TcpStream>;
    let mut config = &config_map.config;
    let conf = &config.get(0).unwrap().config;
    let conf = conf.as_any().downcast_ref::<Config>().unwrap();
    if conf.tls_active {
        http = HttpConn::ssl_new(stream, &mut read_buffer, &mut body_buffer).await;
    } else {
        http = HttpConn::new(
            RawStream::Normal(stream),
            &mut read_buffer,
            &mut body_buffer,
        )
        .await;
    }

    http.parse_first_line().await.unwrap();

    http.parse_headers().await.unwrap();

    // TODO: Read the body only if needed
    match http.read_body().await {
        Ok(_) => (),
        Err(ref e) if e.kind() == ErrorKind::NotFound => (),
        Err(e) => panic!("Error reading request body: {}", e),
    };

    http.response.set_header("server", "jequi");
    http.response.set_header(
        "date",
        &Utc::now().format("%a, %e %b %Y %T GMT").to_string(),
    );

    let mut uri_map = &config_map.uri;
    if let Some(host_map) = &config_map.host 
    && let Some(host) = &http.request.host 
    && let Some(host_config) = host_map.get(host) {
       config = &host_config.config; 
       uri_map = &host_config.uri;
    }

    let get_config_from_uri = || -> &Vec<Plugin> {
        if let Some(uri_map) = uri_map && !uri_map.is_empty() {
            let mut uri: &str = &http.request.uri;
            if let Some(i) = uri.find('?') {
                uri = &uri[..i];
            }
            if let Some(config) = uri_map.get(uri) {
                return config;
            }
            while let Some(i) = uri.rfind('/') {
                uri = &uri[..i];
                if let Some(config) = uri_map.get(uri) {
                    return config;
                }
            }
        }
        config
    };
    config = get_config_from_uri();

    for handler in config.iter().map(|x| &x.request_handler).flatten() {
        handler.handle_request(&mut http.request, &mut http.response);
    }

    if http.response.status == 0 {
        http.response.status = 200;
    }

    http.write_response().await.unwrap();
}

async fn listen_reload(plugins: Arc<RwLock<Arc<ConfigMap>>>) {
    let mut stream = signal(SignalKind::hangup()).unwrap();

    // Print whenever a HUP signal is received
    loop {
        stream.recv().await;
        println!("Reload");
        let loaded = load_config();
        *plugins.write().await = Arc::new(loaded);
    }
}

fn merge_yaml(a: &mut serde_yaml::Value, b: serde_yaml::Value) {
    match (a, b) {
        (serde_yaml::Value::Mapping(ref mut a), serde_yaml::Value::Mapping(b)) => {
            for (k, v) in b {
                if let Some(b_seq) = v.as_sequence()
                    && let Some(a_val) = a.get(&k)
                    && let Some(a_seq) = a_val.as_sequence()
                {
                    a[&k] = [a_seq.as_slice(), b_seq.as_slice()].concat().into();
                    continue;
                }

                if !a.contains_key(&k) {
                    a.insert(k, v);
                } else {
                    merge_yaml(&mut a[&k], v);
                }
            }
        }
        (a, b) => *a = b,
    }
}

fn load_config() -> ConfigMap {
    let mut main_conf = ConfigMap::default();
    let main_conf_parser = ConfigMapParser::load_config("conf.yaml");

    let config_parser = main_conf_parser.config.clone();
    for (host, host_config_parser) in main_conf_parser.host.into_iter().flatten() {
        let mut config_parser = config_parser.clone();
        merge_yaml(&mut config_parser, host_config_parser.config);
        let plugin_list = plugins::load_configs(&config_parser);
        let mut uri_config: Option<HashMap<String, Vec<Plugin>>> = None;
        for (uri, uri_config_parser) in host_config_parser.uri.into_iter().flatten() {
            let mut config_parser = config_parser.clone();
            merge_yaml(&mut config_parser, uri_config_parser);
            let plugin_list = plugins::load_configs(&config_parser);
            uri_config.get_or_insert_default().insert(uri, plugin_list);
        }
        main_conf.host.get_or_insert_default().insert(
            host,
            jequi::HostConfig {
                uri: uri_config,
                config: plugin_list,
            },
        );
    }
    for (uri, uri_config) in main_conf_parser.uri.into_iter().flatten() {
        let mut config_parser = config_parser.clone();
        merge_yaml(&mut config_parser, uri_config);
        let plugin_list = plugins::load_configs(&config_parser);
        main_conf.uri.get_or_insert_default().insert(uri, plugin_list);
    }

    main_conf.config = plugins::load_configs(&config_parser);
    main_conf
}

#[tokio::main]
async fn main() {
    fs::write("./jequi.pid", process::id().to_string()).unwrap();

    let loaded = load_config();
    let config = Arc::new(RwLock::new(Arc::new(loaded)));

    let conf = config.read().await.config.get(0).unwrap().config.clone();
    let conf = conf.as_any().downcast_ref::<Config>().unwrap();

    let address = (conf.ip.clone(), conf.port);

    spawn(listen_reload(config.clone()));

    let listener = TcpListener::bind(address).await.unwrap();

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let config = config.read().await.clone();
        tokio::spawn(async move {
            handle_connection(stream, config).await;
        });
    }
}
