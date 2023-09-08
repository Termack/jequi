use std::{fs, io::ErrorKind, sync::Arc};

use tokio::{
    net::{TcpListener, TcpStream},
    signal::unix::{signal, SignalKind},
    spawn,
    sync::RwLock,
};

use jequi::{Config, HttpConn, JequiConfig, RawStream, RequestHandler};

use plugins;

use chrono::Utc;

use std::process;

async fn handle_connection(
    stream: TcpStream,
    configs: Arc<Vec<Arc<dyn JequiConfig>>>,
    handlers: Arc<Vec<Arc<dyn RequestHandler>>>,
) {
    let mut read_buffer = [0; 1024];
    let mut body_buffer = [0; 1024];
    let mut http: HttpConn<'_, TcpStream>;
    let conf = configs.get(0).unwrap().to_owned();
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

    for handler in handlers.iter() {
        handler.handle_request(&mut http.request, &mut http.response);
    }

    if http.response.status == 0 {
        http.response.status = 200;
    }

    http.write_response().await.unwrap();
}

async fn listen_reload(
    configs: Arc<RwLock<Arc<Vec<Arc<dyn JequiConfig>>>>>,
    handlers: Arc<RwLock<Arc<Vec<Arc<dyn RequestHandler>>>>>,
) {
    let mut stream = signal(SignalKind::hangup()).unwrap();

    // Print whenever a HUP signal is received
    loop {
        stream.recv().await;
        println!("Reload");
        let loaded = load_config();
        *configs.write().await = Arc::new(loaded.0);
        *handlers.write().await = Arc::new(loaded.1);
    }
}

fn load_config() -> (Vec<Arc<dyn JequiConfig>>, Vec<Arc<dyn RequestHandler>>) {
    let main_conf = jequi::config::load_config("conf.yaml");

    plugins::load_configs(main_conf)
}

#[tokio::main]
async fn main() {
    fs::write("./jequi.pid", process::id().to_string()).unwrap();

    let loaded = load_config();
    let (configs, handlers) = (
        Arc::new(RwLock::new(Arc::new(loaded.0))),
        Arc::new(RwLock::new(Arc::new(loaded.1))),
    );

    let conf = configs.read().await.get(0).unwrap().to_owned();
    let conf = conf.as_any().downcast_ref::<Config>().unwrap();

    let address = (conf.ip.clone(), conf.port);

    spawn(listen_reload(configs.clone(), handlers.clone()));

    let listener = TcpListener::bind(address).await.unwrap();

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let configs = configs.read().await.clone();
        let handlers = handlers.read().await.clone();
        tokio::spawn(async move {
            handle_connection(stream, configs, handlers).await;
        });
    }
}
