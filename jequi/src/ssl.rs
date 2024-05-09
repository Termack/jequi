use plugins::get_plugin;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};

use openssl::pkey::PKey;
use openssl::ssl::{
    AlpnError, NameType, SniError, Ssl, SslAcceptor, SslAlert, SslContextBuilder, SslMethod, SslRef,
};
use openssl::x509::X509;

use tokio_openssl::SslStream;

use crate::{ConfigMap, RawStream};

use crate as jequi;

static INTERMEDIATE_CERT: &[u8] = include_bytes!("../test/intermediate.pem");
static LEAF_CERT: &[u8] = include_bytes!("../test/leaf-cert.pem");
static LEAF_KEY: &[u8] = include_bytes!("../test/leaf-cert.key");

pub async fn ssl_new<T: AsyncRead + AsyncWrite + Unpin + Send>(
    stream: T,
    config_map: Arc<ConfigMap>,
) -> (RawStream<T>, String) {
    let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    acceptor.set_servername_callback(
        move |ssl_ref: &mut SslRef, _ssl_alert: &mut SslAlert| -> Result<(), SniError> {
            let key = PKey::private_key_from_pem(LEAF_KEY).unwrap();
            let cert = X509::from_pem(LEAF_CERT).unwrap();
            let intermediate = X509::from_pem(INTERMEDIATE_CERT).unwrap();

            let mut ctx_builder = SslContextBuilder::new(SslMethod::tls()).unwrap();
            let config =
                config_map.get_config_for_request(ssl_ref.servername(NameType::HOST_NAME), None);
            let conf = get_plugin!(config, jequi).unwrap();
            let http2 = conf.http2;

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

    (RawStream::Ssl(stream), version)
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;
    use std::sync::Arc;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    use openssl::ssl::{SslConnector, SslMethod};
    use tokio_openssl::SslStream;

    use crate::{http1::Http1Conn, Config, ConfigMap, Plugin, RawStream, RequestHandler};

    static ROOT_CERT_PATH: &str = "test/root-ca.pem";

    #[tokio::test]
    async fn ssl_handshake_test() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let stream = listener.accept().await.unwrap().0;

            let mut main_conf = ConfigMap::default();
            main_conf.config.push(Plugin {
                config: Arc::new(Config::default()),
                request_handler: RequestHandler(None),
            });

            let (stream, _) = super::ssl_new(stream, Arc::new(main_conf)).await;
            let req = Http1Conn::new(stream);

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
