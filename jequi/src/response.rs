use std::{
    cmp,
    io::{Error, ErrorKind, Result},
};

use crate::{HttpConn, Response};
use http::{header, HeaderMap, HeaderName, HeaderValue};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

impl<'a, T: AsyncRead + AsyncWrite + Unpin> HttpConn<T> {
    pub async fn write_response(&mut self) -> Result<()> {
        let mut headers = String::new();
        let status_line = format!("{} {}\n", self.version, self.response.status);
        headers += &status_line;
        if let Some(encoding) = self.response.get_header(header::TRANSFER_ENCODING.as_str()) && encoding.to_str().unwrap().trim().eq_ignore_ascii_case("chunked") {
        } else {
            let content_length = self.response.body_buffer.len();
            self.response
                .set_header("content-length", &content_length.to_string());
        }
        for (key, value) in &self.response.headers {
            let header = format!("{}: {}\n", key, value.to_str().unwrap());
            headers += &header;
        }
        headers += "\n";
        self.stream.write_all(headers.as_bytes()).await?;
        self.stream.write_all(&self.response.body_buffer).await?;
        self.stream.flush().await?;
        Ok(())
    }
}

impl Response {
    pub fn new() -> Response {
        Response {
            status: 0,
            headers: HeaderMap::new(),
            body_buffer: Vec::new(),
        }
    }

    pub fn set_header(&mut self, header: &str, value: &str) -> Option<HeaderValue> {
        self.headers.insert(
            header.trim().to_lowercase().parse::<HeaderName>().unwrap(),
            value.parse().unwrap(),
        )
    }

    pub fn get_header(&mut self, header: &str) -> Option<&HeaderValue> {
        self.headers.get(header.to_lowercase().trim())
    }

    pub fn write_body(&mut self, bytes: &[u8]) -> Result<()> {
        self.body_buffer.extend_from_slice(bytes);
        Ok(())
    }
}

impl Default for Response {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use http::HeaderMap;
    use tokio::io::{AsyncReadExt, BufStream};

    use crate::{HttpConn, RawStream, Request, Response};

    fn new_response(
        headers: HeaderMap,
        status: usize,
        version: String,
        body: Vec<u8>,
    ) -> HttpConn<Cursor<Vec<u8>>> {
        let stream: Vec<u8> = Vec::new();
        HttpConn {
            stream: BufStream::new(RawStream::Normal(Cursor::new(stream))),
            version,
            request: Request {
                method: String::new(),
                uri: String::new(),
                headers: HeaderMap::new(),
                host: None,
                body: None,
            },
            response: Response {
                status,
                headers,
                body_buffer: body,
            },
        }
    }

    #[tokio::test]
    async fn response_write_test() {
        let bodies = (
            Vec::from("hello world"),
            Vec::from("test2 2 2 2 2"),
            Vec::from("blaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n".to_owned())
        );
        let responses_in: Vec<HttpConn<Cursor<Vec<u8>>>> =
            vec![
            new_response(
                HeaderMap::from_iter([
                    ("server".parse().unwrap(), "jequi".parse().unwrap()),
                    ("content-type".parse().unwrap(), "application/json".parse().unwrap()),
                ]),
                301,
                "HTTP/1.1".to_string(),
                bodies.0,
            ),
            new_response(
                HeaderMap::from_iter([
                    ("cache-control".parse().unwrap(), "max-age=1296000".parse().unwrap()),
                    (
                        "strict-transport-security".parse().unwrap(),
                        "max-age=31536000".parse().unwrap(),
                    ),
                ]),
                200,
                "HTTP/2".to_string(),
                bodies.1,
            ),
            new_response(
                HeaderMap::from_iter([
                    ("content-length".parse().unwrap(), "1565".parse().unwrap()),
                    (
                        "set-cookie".parse().unwrap(),
                        "PHPSESSID=bla; path=/; domain=.example.com;HttpOnly;Secure;SameSite=None"
                            .parse().unwrap(),
                    ),
                ]),
                404,
                "HTTP/1".to_string(),
                bodies.2,
            ),
        ];

        let expected_responses: Vec<&[u8]> = vec![
            b"\
HTTP/1.1 301
server: jequi
content-type: application/json
content-length: 11

hello world",
            b"\
HTTP/2 200
cache-control: max-age=1296000
strict-transport-security: max-age=31536000
content-length: 13

test2 2 2 2 2",
            b"\
HTTP/1 404
content-length: 86
set-cookie: PHPSESSID=bla; path=/; domain=.example.com;HttpOnly;Secure;SameSite=None

blaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
",
        ];
        for (i, mut r) in responses_in.into_iter().enumerate() {
            r.write_response().await.unwrap();

            let mut buf = Vec::new();

            if let RawStream::Normal(stream) = r.stream.get_mut() {
                stream.set_position(0)
            }

            let n = r.stream.read_to_end(&mut buf).await.unwrap();

            assert_eq!(
                String::from_utf8_lossy(&buf[..n]),
                String::from_utf8_lossy(expected_responses[i])
            )
        }
    }

    #[test]
    fn get_set_header_test() {
        let mut http = new_response(
            HeaderMap::from_iter([("server".parse().unwrap(), "not-jequi".parse().unwrap())]),
            0,
            String::new(),
            Vec::new(),
        );

        http.response.set_header("server", "jequi");
        let server = http.response.get_header("server");
        assert_eq!("jequi", server.unwrap().to_str().unwrap());

        http.response.set_header("Content-Length", "40");
        let server = http.response.get_header("content-length");
        assert_eq!("40", server.unwrap().to_str().unwrap());
    }

    #[tokio::test]
    async fn test_write_body() {
        let stream = Cursor::new(Vec::new());
        let mut http = HttpConn::new(RawStream::Normal(stream));
        let resp = &mut http.response;
        resp.write_body(b"hello").unwrap();
        assert_eq!(b"hello", &resp.body_buffer[..]);
        resp.write_body(b" world").unwrap();
        assert_eq!(b"hello world", &resp.body_buffer[..]);
    }
}
