use std::fmt::Debug;
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncWrite};

use openssl::pkey::PKey;
use openssl::ssl::{SniError, Ssl, SslAcceptor, SslAlert, SslContextBuilder, SslMethod, SslRef};
use openssl::x509::X509;

use tokio_openssl::SslStream;

use crate::{HttpConn, RawStream};

static INTERMEDIATE_CERT: &[u8] = include_bytes!("../test/intermediate.pem");
static LEAF_CERT: &[u8] = include_bytes!("../test/leaf-cert.pem");
static LEAF_KEY: &[u8] = include_bytes!("../test/leaf-cert.key");

impl<'a, T: AsyncRead + AsyncWrite + Debug + Unpin> HttpConn<T> {
    pub async fn ssl_new(stream: T) -> HttpConn<T> {
        let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        acceptor.set_servername_callback(
            |ssl_ref: &mut SslRef, _ssl_alert: &mut SslAlert| -> Result<(), SniError> {
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
        let acceptor = acceptor.build();

        let ssl = Ssl::new(acceptor.context()).unwrap();
        let mut stream = SslStream::new(ssl, stream).unwrap();

        Pin::new(&mut stream).accept().await.unwrap();
        println!("{:?}", stream.ssl().selected_alpn_protocol());

        HttpConn::new(RawStream::Ssl(stream))
    }
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    use openssl::ssl::{SslConnector, SslMethod};
    use tokio_openssl::SslStream;

    use crate::{HttpConn, RawStream};

    static ROOT_CERT_PATH: &str = "test/root-ca.pem";

    #[tokio::test]
    async fn ssl_handshake_test() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let stream = listener.accept().await.unwrap().0;

            let req = HttpConn::ssl_new(stream).await;

            if let RawStream::Ssl(mut stream) = req.stream.into_inner() {
                stream.write_all(b"hello").await.unwrap()
            } else {
                panic!("Stream is not ssl")
            }
        });

        let mut connector = SslConnector::builder(SslMethod::tls()).unwrap();
        connector.set_ca_file(ROOT_CERT_PATH).unwrap();
        let ssl = connector
            .build()
            .configure()
            .unwrap()
            .into_ssl("localhost")
            .unwrap();

        let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let mut stream = SslStream::new(ssl, stream).unwrap();

        Pin::new(&mut stream).connect().await.unwrap();

        let mut buf = [0; 5];
        stream.read_exact(&mut buf).await.unwrap();
        assert_eq!(b"hello", &buf);
    }
}
