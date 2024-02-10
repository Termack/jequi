use std::io::Cursor;

use http::HeaderMap;

use crate::RawStream;

use super::*;

#[tokio::test]
async fn parse_first_line_test() {
    let requests_in: Vec<Vec<u8>> = vec![
        Vec::from("GET / HTTP/1.1 \n"),
        Vec::from("POST /bla HTTP/2.0\n"),
        Vec::from("PUT  /abcd HTTP/1.2\n"),
        Vec::from("GET  /aadfsdab  HTTP/1.1 \n"),
    ];

    #[derive(Debug, PartialEq)]
    struct Result {
        method: String,
        uri: String,
        version: String,
    }

    fn new_result(method: String, uri: String, version: String) -> Result {
        Result {
            method,
            uri,
            version,
        }
    }

    let expected_results = vec![
        new_result("GET".to_string(), "/".to_string(), "HTTP/1.1".to_string()),
        new_result(
            "POST".to_string(),
            "/bla".to_string(),
            "HTTP/2.0".to_string(),
        ),
        new_result(
            "PUT".to_string(),
            "/abcd".to_string(),
            "HTTP/1.2".to_string(),
        ),
        new_result(
            "GET".to_string(),
            "/aadfsdab".to_string(),
            "HTTP/1.1".to_string(),
        ),
    ];

    for (i, r) in requests_in.iter().enumerate() {
        let mut req = HttpConn::new(RawStream::Normal(Cursor::new(r.clone())));

        let err = req.parse_first_line().await;

        err.expect(&format!("{:?}", String::from_utf8_lossy(r)));

        assert_eq!(
            expected_results[i],
            new_result(req.request.method, req.request.uri, req.version,),
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
PUT  /abcd HTTP/1.2
Host: host.com
Cookies: aa=bb

"
        .to_vec(),
        b"\
  GET  /aadfsdab  HTTP/1.1
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

    let expected_results: Vec<HeaderMap> = vec![
        HeaderMap::from_iter([
            ("host".parse().unwrap(), "example.com".parse().unwrap()),
            (
                "content-type".parse().unwrap(),
                "application/json".parse().unwrap(),
            ),
        ]),
        HeaderMap::from_iter([
            ("user-agent".parse().unwrap(), "Mozilla".parse().unwrap()),
            ("accept-encoding".parse().unwrap(), "gzip".parse().unwrap()),
        ]),
        HeaderMap::from_iter([
            ("host".parse().unwrap(), "host.com".parse().unwrap()),
            ("cookies".parse().unwrap(), "aa=bb".parse().unwrap()),
        ]),
        HeaderMap::from_iter([
            ("bla".parse().unwrap(), "bla".parse().unwrap()),
            ("ble".parse().unwrap(), "ble".parse().unwrap()),
            ("header".parse().unwrap(), "haaa".parse().unwrap()),
            ("wowo".parse().unwrap(), "10034mc amk".parse().unwrap()),
        ]),
        HeaderMap::from_iter([("content-length".parse().unwrap(), "11".parse().unwrap())]),
    ];

    for (i, r) in requests_in.iter().enumerate() {
        let mut req = HttpConn::new(RawStream::Normal(Cursor::new(r.clone())));

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

    let expected_results: Vec<Result<Vec<u8>>> = vec![
        Ok("hello world".into()),
        Ok("".into()),
        Ok("12345".into()),
        Err(Error::new(
            ErrorKind::UnexpectedEof,
            "early eof",
        )),
        Err(Error::new(
            ErrorKind::Other,
            format!("Cant convert content length to int: invalid digit found in string"),
        )),
        Ok("vpm1DH8sIat11ezv8GulW93nT7uTxVF5RH58WH7INSMvvzqXSd3O6Np11MOcI8gVXVpKOSwNsCQusuMyfjZ5eXC6eD7sQdRal\nr\n".into()),
    ];

    for (i, r) in requests_in.iter().enumerate() {
        let mut req = HttpConn::new(RawStream::Normal(Cursor::new(r.clone())));

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
