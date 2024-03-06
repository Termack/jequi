use http::version;
use std::fmt::Debug;
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};

use openssl::pkey::PKey;
use openssl::ssl::{
    AlpnError, SniError, Ssl, SslAcceptor, SslAlert, SslContextBuilder, SslMethod, SslRef,
};
use openssl::x509::X509;

use tokio_openssl::SslStream;

use crate::{HttpConn, RawStream};

static INTERMEDIATE_CERT: &[u8] = include_bytes!("../test/intermediate.pem");
static LEAF_CERT: &[u8] = include_bytes!("../test/leaf-cert.pem");
static LEAF_KEY: &[u8] = include_bytes!("../test/leaf-cert.key");

impl<'a, T: AsyncRead + AsyncWrite + Debug + Unpin + Send> HttpConn<T> {
    pub async fn ssl_new(stream: T, http2: bool) -> HttpConn<T> {
        let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        acceptor.set_servername_callback(
            move |ssl_ref: &mut SslRef, _ssl_alert: &mut SslAlert| -> Result<(), SniError> {
                let key = PKey::private_key_from_pem(LEAF_KEY).unwrap();
                let cert = X509::from_pem(LEAF_CERT).unwrap();
                let intermediate = X509::from_pem(INTERMEDIATE_CERT).unwrap();

                let mut ctx_builder = SslContextBuilder::new(SslMethod::tls()).unwrap();

                ctx_builder.set_private_key(&key).unwrap();
                ctx_builder.set_certificate(&cert).unwrap();
                ctx_builder.add_extra_chain_cert(intermediate).unwrap();
                ctx_builder.set_alpn_protos(b"\x02h2\x08http/1.1").unwrap();
                ctx_builder.set_alpn_select_callback(move |_, protos| {
                    if !http2 {
                        if protos.windows(9).any(|window| window == b"\x08http/1.1") {
                            return Ok(b"http/1.1");
                        }
                        return Err(AlpnError::NOACK);
                    }
                    if protos.windows(3).any(|window| window == b"\x02h2") {
                        Ok(b"h2")
                    } else if protos.windows(9).any(|window| window == b"\x08http/1.1") {
                        Ok(b"http/1.1")
                    } else {
                        Err(AlpnError::NOACK)
                    }
                });

                ssl_ref.set_ssl_context(&ctx_builder.build()).unwrap();
                Ok(())
            },
        );

        let acceptor = acceptor.build();

        let ssl = Ssl::new(acceptor.context()).unwrap();
        let mut stream = SslStream::new(ssl, stream).unwrap();

        Pin::new(&mut stream).accept().await.unwrap();

        let version = match stream.ssl().selected_alpn_protocol() {
            Some(protocol) => String::from_utf8_lossy(protocol).to_string(),
            None => String::new(),
        };

        HttpConn::with_version(RawStream::Ssl(stream), version)
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

            let req = HttpConn::ssl_new(stream, false).await;

            if let RawStream::Ssl(mut stream) = req.conn.into_inner() {
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
