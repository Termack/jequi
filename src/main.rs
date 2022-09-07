use std::net::{TcpStream, TcpListener};

use jequi::request;

fn handle_connection(stream: TcpStream) {
    let mut buffer = [0;1024];
    let tls_active = true;
    let mut req;
    if tls_active {
        req = request::Request::ssl_new(stream, &mut buffer);
    }else {
        req = request::Request::new(stream, &mut buffer);
    }

    req.parse_first_line().unwrap();

    println!("{} {} {}",req.method,req.uri,req.version);
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        handle_connection(stream);
    }
}
