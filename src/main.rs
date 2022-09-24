use std::net::{TcpStream, TcpListener};

use jequi::{HttpConn, RawStream};

use chrono::Utc;

fn handle_connection(stream: TcpStream) {
    let mut buffer = [0;1024];
    let tls_active = true;
    let mut http;
    if tls_active {
        http = HttpConn::ssl_new(stream, &mut buffer);
    }else {
        http = HttpConn::new(RawStream::Normal(stream), &mut buffer);
    }

    http.parse_first_line().unwrap();

    http.response.set_header("server", "jequi");
    http.response.set_header("date", &Utc::now().format("%a, %e %b %Y %T GMT").to_string());
    http.response.status = 200;

    http.write_response().unwrap();

    println!("method:{} uri:{} version:{}",http.request.method,http.request.uri,http.version);
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        handle_connection(stream);
    }
}
