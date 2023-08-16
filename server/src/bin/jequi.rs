use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};

use jequi::{HttpConn, RawStream, Config};

use plugins;

use chrono::Utc;

async fn handle_connection(config: Arc<Config>,stream: TcpStream) {
    let mut read_buffer = [0; 1024];
    let mut body_buffer = [0; 1024];
    let mut http;
    if config.tls_active {
        http = HttpConn::ssl_new(
            stream,
            &mut read_buffer,
            &mut body_buffer)
            .await;
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

    http.response.set_header("server", "jequi");
    http.response.set_header(
        "date",
        &Utc::now().format("%a, %e %b %Y %T GMT").to_string(),
    );

    if let Some(path) = &config.static_files_path {
        plugins::handle_static_files(&mut http,path)
    }

    plugins::go_handle_response(&mut http.response);

    if http.response.status == 0 {
        http.response.status = 200;
        http.response.write_body(b"hello world\n").unwrap();
    }

    http.write_response().await.unwrap();

    println!(
        "method:{} uri:{} version:{}",
        http.request.method, http.request.uri, http.version
    );
}

#[tokio::main]
async fn main() {
    let config = Arc::new(Config::load_config("./conf.yaml"));

    let address = (config.ip.clone(),config.port.clone());

    let listener = TcpListener::bind(address).await.unwrap();

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let conf = Arc::clone(&config);
        tokio::spawn(async move {
            handle_connection(conf,stream).await;
        });
    }
}
