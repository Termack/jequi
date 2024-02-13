#![feature(let_chains)]
use chrono::Utc;
use jequi::{Config, ConfigMap, HttpConn, RawStream, Request, Response};
use plugins::{get_plugin, load_plugins};
use std::process;
use std::{fs, io::ErrorKind, sync::Arc};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::{
    net::{TcpListener, TcpStream},
    signal::unix::{signal, SignalKind},
    spawn,
    sync::RwLock,
};

load_plugins!();

async fn handle_request<T: AsyncRead + AsyncWrite + Unpin>(
    conf: &Config,
    http: &mut HttpConn<T>,
    config_map: Arc<ConfigMap>,
) {
    http.parse_first_line().await.unwrap();

    http.parse_headers().await.unwrap();

    // TODO: Read the body only if needed (remember to consume stream if body not read)
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

    let config = config_map.get_config_for_request(http.request.host.as_deref(), &http.request.uri);

    for handle_request in config.iter().map(|x| &x.request_handler.0).flat_map(|x| x) {
        if let Some(fut) = handle_request(&mut http.request, &mut http.response) {
            fut.await
        }
    }

    if http.response.status == 0 {
        http.response.status = 200;
    }

    http.write_response(conf.chunk_size).await.unwrap();
}

async fn handle_connection(stream: TcpStream, config_map: Arc<ConfigMap>) {
    let mut http: HttpConn<TcpStream>;
    let plugin_list = &config_map.config;
    let conf = get_plugin!(plugin_list, jequi);
    if conf.tls_active {
        http = HttpConn::ssl_new(stream).await;
    } else {
        http = HttpConn::new(RawStream::Normal(stream));
    }

    handle_request(conf, &mut http, config_map.clone()).await;
    if let Some(connection) = http.request.headers.get("connection") && connection.to_str().unwrap().to_lowercase() == "keep-alive" {
        loop {
            http.request = Request::new();
            http.response = Response::new();
            handle_request(conf, &mut http, config_map.clone()).await;
        }
    }
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
