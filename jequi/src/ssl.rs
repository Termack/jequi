use core::fmt;
use plugins::get_plugin;
use serde::{de, Deserialize};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};

use openssl::pkey::{PKey, Private};
use openssl::ssl::{
    AlpnError, NameType, SniError, Ssl, SslAcceptor, SslAlert, SslContextBuilder, SslMethod, SslRef,
};
use openssl::x509::X509;

use tokio_openssl::SslStream;

use crate::{AsyncRWSend, AsyncRWSendBuf, ConfigMap};

use crate as jequi;

#[derive(Clone, Debug)]
pub struct SslKeyConfig(PKey<Private>);

impl<'de> Deserialize<'de> for SslKeyConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SslKeyConfigVisitor;

        impl<'de> de::Visitor<'de> for SslKeyConfigVisitor {
            type Value = SslKeyConfig;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("SslKeyConfig")
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_str(&v)
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let path = PathBuf::from(v);
                if !path.exists() {
                    return Err(E::custom(format!("path doesn't exist: {}", path.display())));
                }

                let content = std::fs::read(path).unwrap();
                PKey::private_key_from_pem(&content)
                    .map(|key| SslKeyConfig(key))
                    .map_err(|err| E::custom(err.to_string()))
            }
        }

        deserializer.deserialize_string(SslKeyConfigVisitor {})
    }
}

impl PartialEq for SslKeyConfig {
    fn eq(&self, other: &Self) -> bool {
        self.0.public_eq(other.0.as_ref())
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct SslCertConfig(Vec<X509>);

impl<'de> Deserialize<'de> for SslCertConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SslCertConfigVisitor;

        impl<'de> de::Visitor<'de> for SslCertConfigVisitor {
            type Value = SslCertConfig;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("SslKeyConfig")
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_str(&v)
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let path = PathBuf::from(v);
                if !path.exists() {
                    return Err(E::custom(format!("path doesn't exist: {}", path.display())));
                }

                let content = std::fs::read(path).unwrap();
                X509::stack_from_pem(&content)
                    .map(|cert| SslCertConfig(cert))
                    .map_err(|err| E::custom(err.to_string()))
            }
        }

        deserializer.deserialize_string(SslCertConfigVisitor {})
    }
}

pub async fn ssl_new<T: AsyncRWSend>(
    stream: T,
    config_map: Arc<ConfigMap>,
) -> (SslStream<T>, String) {
    let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    acceptor.set_servername_callback(
        move |ssl_ref: &mut SslRef, _ssl_alert: &mut SslAlert| -> Result<(), SniError> {
            // let key = PKey::private_key_from_pem(LEAF_KEY).unwrap();
            // let cert = X509::from_pem(LEAF_CERT).unwrap();
            // let intermediate = X509::from_pem(INTERMEDIATE_CERT).unwrap();

            let mut ctx_builder = SslContextBuilder::new(SslMethod::tls()).unwrap();
            let config =
                config_map.get_config_for_request(ssl_ref.servername(NameType::HOST_NAME), None);
            let conf = get_plugin!(config, jequi).unwrap();
            let http2 = conf.http2;

            ctx_builder
                .set_private_key(&conf.ssl_key.as_ref().unwrap().0)
                .unwrap();
            let mut chain = conf.ssl_certificate.as_ref().unwrap().0.iter();
            ctx_builder.set_certificate(chain.next().unwrap()).unwrap();
            for cert in chain {
                ctx_builder.add_extra_chain_cert(cert.clone()).unwrap();
            }
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

    (stream, version)
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;
    use std::sync::Arc;

    use serde::de::value::{Error, StrDeserializer};
    use serde::de::IntoDeserializer;
    use serde::Deserialize;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    use openssl::ssl::{SslConnector, SslMethod};
    use tokio_openssl::SslStream;

    use crate::ssl::{SslCertConfig, SslKeyConfig};
    use crate::JequiConfig;
    use crate::{http1::Http1Conn, Config, ConfigMap, Plugin, RequestHandler};

    static ROOT_CERT_PATH: &str = "test/root-ca.pem";

    #[tokio::test]
    async fn ssl_handshake_test() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let stream = listener.accept().await.unwrap().0;

            let mut main_conf = ConfigMap::default();

            let conf = Config {
                ssl_certificate: Some(
                    SslCertConfig::deserialize::<StrDeserializer<'_, Error>>(
                        "test/leaf-cert.pem".into_deserializer(),
                    )
                    .unwrap(),
                ),
                ssl_key: Some(
                    SslKeyConfig::deserialize::<StrDeserializer<'_, Error>>(
                        "test/leaf-cert.key".into_deserializer(),
                    )
                    .unwrap(),
                ),
                ..Config::default()
            };
            main_conf.config.push(Plugin {
                config: Arc::new(conf),
                request_handler: RequestHandler(None),
            });

            let (mut stream, _) = super::ssl_new(stream, Arc::new(main_conf)).await;

            stream.write_all(b"hello").await.unwrap();
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
