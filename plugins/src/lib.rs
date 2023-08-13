use tokio::io::{AsyncRead, AsyncWrite};

use jequi::{HttpConn, Response};

#[link(name = "jequi_go")]
extern "C" {
    pub fn HandleResponse(resp: *mut Response);
}

pub fn handle_static_files<'a,T: AsyncRead + AsyncWrite + Unpin>(http: &mut HttpConn<'a, T>, path: &str) {
    println!("yaya")
}