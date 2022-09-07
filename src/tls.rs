use std::collections::HashMap;
use std::fmt::Debug;
use std::io::{Write, Read};
use std::sync::Arc;
use openssl::error::ErrorStack;
use openssl::ssl::{SslAcceptor, SslMethod, SslRef, SslAlert, ClientHelloResponse, NameType, SniError, SslFiletype, SslStream};

use crate::request::{RawRequest, RawStream};

use super::request::Request;

// CHAIN CERTS

fn sni(sslRef: &mut SslRef, sslAlert: &mut SslAlert) -> Result<(), SniError>{
    println!("{:?}",sslRef.state_string_long());
    println!("{:?}",sslRef.version_str());
    println!("{:?}",sslRef.verify_mode());
    println!("{:?}",sslRef.verify_result());
    // println!("{:?}",sslRef.servername(NameType::HOST_NAME).unwrap());
    println!("{:?}",sslRef.servername_raw(NameType::HOST_NAME));
    // sslRef.set_certificate_file("/home/filipe/projects/jequi/test/test.crt",SslFiletype::PEM).unwrap();
    // sslRef.set_certificate_chain_file("/home/filipe/projects/jequi/test/new/localhost.chain").unwrap();
    Ok(())
    // Err(SniError::NOACK)
}

fn client_hello(sslRef: &mut SslRef, sslAlert: &mut SslAlert) -> Result<ClientHelloResponse, ErrorStack> {
    println!("{:?}",sslRef.client_hello_ciphers());
    println!("{:?}",sslRef.version_str());
    println!("{:?}",sslRef.verify_mode());
    println!("{:?}",sslRef.verify_result());
    println!("{:?}",sslRef.state_string_long());
    println!("{:?}",sslRef.servername_raw(NameType::HOST_NAME));
    Ok(ClientHelloResponse::SUCCESS)
    // Err(ErrorStack::get())
}

fn handle_client<T: Read + Write>(mut stream: SslStream<&mut Box<T>>) {
    stream.write(b"\
HTTP/1.1 200
date: Sun, 04 Sep 2022 21:27:58 GMT
content-type: application/octet-stream
server: gocache

").unwrap();
    stream.shutdown().unwrap();
}

impl<'a, T: Read + Write + Debug> Request<'a, T> {
    pub fn ssl_new(stream: T,buffer: &mut [u8]) -> Request<T> {
        let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        // https://www.openssl.org/docs/manmaster/man3/SSL_use_certificate.html
        acceptor.set_private_key_file("/home/filipe/projects/jequi/test/new/localhost.key", SslFiletype::PEM).unwrap();
        acceptor.set_certificate_chain_file("/home/filipe/projects/jequi/test/new/localhost.chain").unwrap();
        acceptor.set_verify_depth(5);
        // acceptor.check_private_key().unwrap();
        acceptor.set_client_hello_callback(client_hello);
        acceptor.set_servername_callback(sni);
        // acceptor.set_cipher_list("DEFAULT").unwrap();
        // acceptor.set_ciphersuites("TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256:TLS_AES_128_GCM_SHA256").unwrap();
        let acceptor = Arc::new(acceptor.build());
        let stream = acceptor.accept(stream).unwrap();
        
        Request{
            raw:RawRequest{
                stream: RawStream::Ssl(stream),
                buffer,
                start: 0,
                end: 0,
            },
            method:String::new(),
            uri:String::new(),
            version:String::new(),
            headers:HashMap::new()
        }
    }
}