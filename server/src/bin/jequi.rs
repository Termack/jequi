use std::{sync::Arc, fs, io::ErrorKind};

use tokio::{
    net::{TcpListener, TcpStream},
    signal::unix::{signal, SignalKind},
    spawn,
    sync::RwLock,
};

use jequi::{Config, HttpConn, RawStream};

use plugins;

use chrono::Utc;

use std::process;

async fn handle_connection(config: Config, stream: TcpStream) {
    let mut read_buffer = [0; 1024];
    let mut body_buffer = [0; 1024];
    let mut http;
    if config.tls_active {
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
        Ok(_)=>(),
        Err(ref e) if e.kind() == ErrorKind::NotFound => (),
        Err(e) => panic!("Error reading request body: {}",e)
    };

    http.response.set_header("server", "jequi");
    http.response.set_header(
        "date",
        &Utc::now().format("%a, %e %b %Y %T GMT").to_string(),
    );

    if let Some(path) = &config.static_files_path {
        plugins::handle_static_files(&mut http, path)
    }

    if let Some(lib_path) = &config.go_library_path {
        plugins::go_handle_request(&mut http.request,&mut http.response, lib_path);
    }

    if http.response.status == 0 {
        http.response.status = 200;
    }

    http.write_response().await.unwrap();
}

async fn listen_reload(config: Arc<RwLock<Config>>) {
    let mut stream = signal(SignalKind::hangup()).unwrap();

    // Print whenever a HUP signal is received
    loop {
        stream.recv().await;
        println!("Reload");
        let lib_path = &config.read().await.go_library_path.clone();
        config.write().await.go_library_path = Some(plugins::load_go_lib(lib_path));
    }
}

#[tokio::main]
async fn main() {
    fs::write("./jequi.pid",process::id().to_string()).unwrap();

    let mut config = Arc::new(RwLock::new(Config::load_config("./conf.yaml")));

    {
        config.write().await.go_library_path = Some(plugins::load_go_lib(&None));
    }

    let address = (config.read().await.ip.clone(), config.read().await.port);

    spawn(listen_reload(Arc::clone(&mut config)));

    let listener = TcpListener::bind(address).await.unwrap();

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let conf = config.read().await.clone();
        tokio::spawn(async move {
            handle_connection(conf, stream).await;
        });
    }
}
