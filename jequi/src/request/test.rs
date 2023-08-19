use std::io::Cursor;

use indexmap::IndexMap;

use crate::RawStream;

use super::*;

#[tokio::test]
async fn parse_first_line_test() {
    let requests_in: Vec<Vec<u8>> = vec![
        Vec::from("GET / HTTP/1.1 \n"),
        Vec::from("POST /bla HTTP/2.0\n"),
        Vec::from("PUT  /ab cd HTTP/1.2\n"),
        Vec::from("  GET  / a adfsdab  HTTP/1.1 \n"),
    ];

    #[derive(Debug, PartialEq)]
    struct Result {
        method: String,
        uri: String,
        version: String,
        index: usize,
    }

    fn new_result(method: String, uri: String, version: String, index: usize) -> Result {
        Result {
            method,
            uri,
            version,
            index,
        }
    }

    let expected_results = vec![
        new_result(
            "GET".to_string(),
            "/".to_string(),
            "HTTP/1.1".to_string(),
            16,
        ),
        new_result(
            "POST".to_string(),
            "/bla".to_string(),
            "HTTP/2.0".to_string(),
            19,
        ),
        new_result(
            "PUT".to_string(),
            "/ab cd".to_string(),
            "HTTP/1.2".to_string(),
            21,
        ),
        new_result(
            "GET".to_string(),
            "/ a adfsdab".to_string(),
            "HTTP/1.1".to_string(),
            30,
        ),
    ];

    for (i, r) in requests_in.iter().enumerate() {
        let mut buf = [0; 35];
        let mut req = HttpConn::new(
            RawStream::Normal(Cursor::new(r.clone())),
            &mut buf,
            &mut [0; 0],
        )
        .await;

        let err = req.parse_first_line().await;

        err.unwrap();

        assert_eq!(
            expected_results[i],
            new_result(
                req.request.method,
                req.request.uri,
                req.version,
                req.raw.start
            ),
            "Testing parse for line: {}",
            String::from_utf8_lossy(r)
        )
    }
}

#[tokio::test]
async fn parse_headers_test() {
    let requests_in: Vec<Vec<u8>> = vec![
        b"\
GET / HTTP/1.1 
Host: example.com
Content-Type: application/json

"
        .to_vec(),
        b"\
POST /bla HTTP/2.0\r
User-Agent: Mozilla\r
Accept-Encoding: gzip\r
\r
"
        .to_vec(),
        b"\
PUT  /ab cd HTTP/1.2
Host: host.com
Cookies: aa=bb

"
        .to_vec(),
        b"\
  GET  / a adfsdab  HTTP/1.1 
Bla: bla
Ble: ble
Header: haaa
Wowo: 10034mc amk

"
        .to_vec(),
        b"\
GET / HTTP/1.1 
Content-Length: 11

hello world"
            .to_vec(),
    ];

    let expected_results: Vec<IndexMap<String, String>> = vec![
        IndexMap::from([
            ("host".to_string(), "example.com".to_string()),
            ("content-type".to_string(), "application/json".to_string()),
        ]),
        IndexMap::from([
            ("user-agent".to_string(), "Mozilla".to_string()),
            ("accept-encoding".to_string(), "gzip".to_string()),
        ]),
        IndexMap::from([
            ("host".to_string(), "host.com".to_string()),
            ("cookies".to_string(), "aa=bb".to_string()),
        ]),
        IndexMap::from([
            ("bla".to_string(), "bla".to_string()),
            ("ble".to_string(), "ble".to_string()),
            ("header".to_string(), "haaa".to_string()),
            ("wowo".to_string(), "10034mc amk".to_string()),
        ]),
        IndexMap::from([("content-length".to_string(), "11".to_string())]),
    ];

    for (i, r) in requests_in.iter().enumerate() {
        let mut buf = [0; 35];
        let mut req = HttpConn::new(
            RawStream::Normal(Cursor::new(r.clone())),
            &mut buf,
            &mut [0; 0],
        )
        .await;

        req.parse_first_line().await.unwrap();

        req.parse_headers().await.unwrap();

        assert_eq!(
            expected_results[i],
            req.request.headers,
            "Testing parse headers for request: {}",
            String::from_utf8_lossy(r)
        )
    }
}

#[tokio::test]
async fn read_body_test() {
    let requests_in: Vec<Vec<u8>> = vec![
        b"\
GET / HTTP/1.1 
Content-Length: 11

hello world"
            .to_vec(),
        b"\
GET / HTTP/1.1 
Content-Length: 0

hello world"
            .to_vec(),
        b"\
GET / HTTP/1.1\r
Content-Length: 5\r
\r
12345"
            .to_vec(),
        b"\
GET / HTTP/1.1\r
Content-Length: 10\r
\r
12345"
            .to_vec(),
        b"\
GET / HTTP/1.1\r
Content-Length: abc\r
\r
12345"
            .to_vec(),
        b"\
GET / HTTP/1.1\r
Content-Length: 100\r
\r
vpm1DH8sIat11ezv8GulW93nT7uTxVF5RH58WH7INSMvvzqXSd3O6Np11MOcI8gVXVpKOSwNsCQusuMyfjZ5eXC6eD7sQdRal
r
"
        .to_vec(),
    ];

    let expected_results: Vec<Result<String>> = vec![
        Ok("hello world".to_string()),
        Ok("".to_string()),
        Ok("12345".to_string()),
        Err(Error::new(
            ErrorKind::InvalidData,
            "Content length and body size dont match",
        )),
        Err(Error::new(
            ErrorKind::Other,
            format!("Cant convert content length to int: invalid digit found in string"),
        )),
        Ok("vpm1DH8sIat11ezv8GulW93nT7uTxVF5RH58WH7INSMvvzqXSd3O6Np11MOcI8gVXVpKOSwNsCQusuMyfjZ5eXC6eD7sQdRal\nr\n".to_string()),
    ];

    for (i, r) in requests_in.iter().enumerate() {
        let mut buf = [0; 40];
        let mut req = HttpConn::new(
            RawStream::Normal(Cursor::new(r.clone())),
            &mut buf,
            &mut [0; 0],
        )
        .await;

        req.parse_first_line().await.unwrap();

        req.parse_headers().await.unwrap();

        let result = req.read_body().await;

        match &expected_results[i] {
            Ok(expected_success) => assert_eq!(
                expected_success,
                &req.request.body.unwrap(),
                "Testing read body for request: {}",
                String::from_utf8_lossy(r)
            ),
            Err(expected_err) => assert_eq!(
                expected_err.to_string(),
                result.unwrap_err().to_string(),
                "Testing read body for request: {}",
                String::from_utf8_lossy(r)
            ),
        }
    }
}
