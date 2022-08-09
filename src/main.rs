use std::net::{TcpStream, TcpListener};

use jequi::request;

fn handle_connection(stream: TcpStream) {
    let mut buffer = [0;10];
    let mut req = request::Request::new(Box::new(stream), &mut buffer);

    req.ssl_hello();

    req.parse_first_line();

    println!("{} {} {}",req.method,req.uri,req.version);
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        handle_connection(stream);
    }
}
