use std::net::{TcpStream, TcpListener};

use jequi::request;

fn handle_connection(stream: TcpStream) {
    let mut req = request::Request::new(Box::new(stream));

    req.parse_first_line();

    println!("{} {} {}",req.method,req.uri,req.version);

    // let status_line = "HTTP/1.1 200 OK";
    // let contents = fs::read_to_string("hello.html").unwrap();

    // let response = format!(
    //     "{}\r\nContent-Length: {}\r\n\r\n{}",
    //     status_line,
    //     contents.len(),
    //     contents
    // );

    // stream.write(response.as_bytes()).unwrap();
    // stream.flush().unwrap();
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        handle_connection(stream);
    }
}
