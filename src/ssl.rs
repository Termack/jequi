use std::fmt::Debug;
use std::io::{Write, Read};
use std::sync::Arc;

use openssl::ssl::{SslAcceptor, SslMethod, SslRef, SslAlert, SniError, SslContextBuilder};
use openssl::pkey::PKey;
use openssl::x509::X509;

use crate::{HttpConn, RawStream};

static INTERMEDIATE_CERT: &[u8] = include_bytes!("../test/intermediate.pem");
static LEAF_CERT: &[u8] = include_bytes!("../test/leaf-cert.pem");
static LEAF_KEY: &[u8] = include_bytes!("../test/leaf-cert.key");

impl<'a, T: Read + Write + Debug> HttpConn<'a, T> {
    pub fn ssl_new(stream: T,buffer: &mut [u8]) -> HttpConn<T> {
        let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        acceptor.set_servername_callback(
            |ssl_ref: &mut SslRef,
             _ssl_alert: &mut SslAlert|
             -> Result<(), SniError> {
                let key = PKey::private_key_from_pem(LEAF_KEY).unwrap();
                let cert = X509::from_pem(LEAF_CERT).unwrap();
                let intermediate = X509::from_pem(INTERMEDIATE_CERT).unwrap();

                let mut ctx_builder = SslContextBuilder::new(SslMethod::tls()).unwrap();

                ctx_builder.set_private_key(&key).unwrap();
                ctx_builder.set_certificate(&cert).unwrap();
                ctx_builder.add_extra_chain_cert(intermediate).unwrap();

                ssl_ref.set_ssl_context(&ctx_builder.build()).unwrap();
                Ok(())
            },
        );
        let acceptor = Arc::new(acceptor.build());
        let stream = acceptor.accept(stream).unwrap();
        
        HttpConn::new(RawStream::Ssl(stream), buffer)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::{net::{TcpStream, TcpListener}, io::Write, thread};

    use openssl::ssl::{SslConnector, SslMethod};

    use crate::{HttpConn, RawStream};

    static ROOT_CERT_PATH: &str = "test/root-ca.pem";

    #[test]
    fn ssl_handshake_test() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let t = thread::spawn(move || {   
            let stream = listener.accept().unwrap().0;
    
            let mut buf = [0;35];
            let req = HttpConn::ssl_new(stream,&mut buf);
            
            if let RawStream::Ssl(mut stream) = req.raw.stream {
                stream.write_all(b"hello").unwrap()
            }else {
                panic!("Stream is not ssl")
            }
        });

        let mut connector = SslConnector::builder(SslMethod::tls()).unwrap();
        connector.set_ca_file(ROOT_CERT_PATH).unwrap();
        let connector = connector.build();
    
        let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let mut stream = connector.connect("localhost", stream).unwrap();
    
        let mut buf = [0; 5];
        stream.read_exact(&mut buf).unwrap();
        assert_eq!(b"hello", &buf);

        t.join().unwrap();
    }
}