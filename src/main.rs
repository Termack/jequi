use std::net::{TcpStream, TcpListener};

use jequi::{HttpConn, RawStream};

fn handle_connection(stream: TcpStream) {
    let mut buffer = [0;1024];
    let tls_active = true;
    let mut req;
    if tls_active {
        req = HttpConn::ssl_new(stream, &mut buffer);
    }else {
        req = HttpConn::new(RawStream::Normal(stream), &mut buffer);
    }

    req.parse_first_line().unwrap();

    println!("{} {} {}",req.request.method,req.request.uri,req.version);
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        handle_connection(stream);
    }
}
