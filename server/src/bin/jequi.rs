use tokio::net::{TcpListener, TcpStream};

use jequi::{HttpConn, RawStream, Response};

use chrono::Utc;

#[link(name = "jequi_go")]
extern "C" {
    fn HandleResponse(resp: *mut Response);
}

async fn handle_connection(stream: TcpStream) {
    let mut read_buffer = [0; 1024];
    let mut body_buffer = [0; 1024];
    let tls_active = true;
    let mut http;
    if tls_active {
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

    http.response.set_header("server", "jequi");
    http.response.set_header(
        "date",
        &Utc::now().format("%a, %e %b %Y %T GMT").to_string(),
    );
    http.response.status = 200;
    http.response.write_body(b"hello world\n").unwrap();

    unsafe { HandleResponse(&mut http.response) };
    http.write_response().await.unwrap();

    println!(
        "method:{} uri:{} version:{}",
        http.request.method, http.request.uri, http.version
    );
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").await.unwrap();

    loop {
        let (stream, _) = listener.accept().await.unwrap();

        tokio::spawn(async move {
            handle_connection(stream).await;
        });
    }
}
