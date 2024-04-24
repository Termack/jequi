#![feature(let_chains)]
use jequi::tcp_stream::new_http_conn;
use jequi::{Config, ConfigMap};
use plugins::load_plugins;
use std::process;
use std::{fs, sync::Arc};
use tokio::{
    net::{TcpListener, TcpStream},
    signal::unix::{signal, SignalKind},
    spawn,
    sync::RwLock,
};

load_plugins!();

async fn handle_connection(stream: TcpStream, config_map: Arc<ConfigMap>) {
    let mut http = new_http_conn(stream, config_map.clone()).await;
    http.handle_connection(config_map).await;
}

async fn listen_reload(config_map: Arc<RwLock<Arc<ConfigMap>>>) {
    let mut stream = signal(SignalKind::hangup()).unwrap();

    // Print whenever a HUP signal is received
    loop {
        stream.recv().await;
        println!("Reload");
        let loaded = ConfigMap::load("conf.yaml", load_plugins);
        *config_map.write().await = Arc::new(loaded);
    }
}

#[tokio::main]
async fn main() {
    fs::write("./jequi.pid", process::id().to_string()).unwrap();

    let config = Arc::new(RwLock::new(Arc::new(ConfigMap::load(
        "conf.yaml",
        load_plugins,
    ))));

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
