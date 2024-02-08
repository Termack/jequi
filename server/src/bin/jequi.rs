use chrono::Utc;
use jequi::{Config, ConfigMap, HttpConn, RawStream};
use plugins::{get_plugin, load_plugins};
use std::process;
use std::{fs, io::ErrorKind, sync::Arc};
use tokio::{
    net::{TcpListener, TcpStream},
    signal::unix::{signal, SignalKind},
    spawn,
    sync::RwLock,
};

load_plugins!();

async fn handle_connection(stream: TcpStream, config_map: Arc<ConfigMap>) {
    let mut read_buffer = [0; 1024];
    let mut body_buffer = [0; 1024 * 256];
    let mut http: HttpConn<'_, TcpStream>;
    let plugin_list = &config_map.config;
    let conf = get_plugin!(plugin_list, jequi);
    if conf.tls_active {
        http = HttpConn::ssl_new(stream, &mut read_buffer, &mut body_buffer).await;
    } else {
        http = HttpConn::new(RawStream::Normal(stream), &mut body_buffer).await;
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

    let config = config_map.get_config_for_request(http.request.host.as_deref(), &http.request.uri);

    for handle_request in config.iter().map(|x| &x.request_handler.0).flat_map(|x| x) {
        if let Some(fut) = handle_request(&mut http.request, &mut http.response) {
            fut.await
        }
    }

    http.response.headers.remove("transfer-encoding");

    if http.response.status == 0 {
        http.response.status = 200;
    }

    http.write_response().await.unwrap();
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
