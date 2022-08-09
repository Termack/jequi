use std::io::{Result, Write, Read};
use std::sync::Arc;
use rustls;
use rustls::server::ClientHello;
use rustls::sign::CertifiedKey;

use super::request;

struct Tls{

}

impl rustls::server::ResolvesServerCert for Tls{
    fn resolve(
        &self, 
        client_hello: ClientHello<'_>
    ) -> Option<Arc<CertifiedKey>>{
        println!("hellooooo");
        println!("{:?}",client_hello.cipher_suites());
        None
    }
}

impl<'a, T: Read + Write> request::Request<'a, T> {
    pub fn ssl_hello(&mut self) -> Result<()> {
        let tls_cfg = {
            // Do not use client certificate authentication.
            let mut cfg = rustls::ServerConfig::builder()
                .with_safe_defaults()
                .with_no_client_auth()
                .with_cert_resolver(Arc::new(Tls{}));
            // Configure ALPN to accept HTTP/2, HTTP/1.1 in that order.
            cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
            Arc::new(cfg)
        };

        let mut tls_conn = rustls::ServerConnection::new(Arc::clone(&tls_cfg)).unwrap();

        let a = tls_conn.read_tls(&mut self.raw.stream);
        let a = tls_conn.process_new_packets();
        println!("{:?} {:?}",a,tls_conn.sni_hostname());
        // let b = tls_conn.write_tls(&mut self.raw.stream);
        // println!("{:?}",b);
        Ok(())
    }
}