use std::{
    cmp,
    io::{Error, ErrorKind, Result},
};

use crate::{HttpConn, Response};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

impl<'a, T: AsyncRead + AsyncWrite + Unpin> HttpConn<'a, T> {
    pub async fn write_response(&mut self) -> Result<()> {
        let mut headers = String::new();
        let status_line = format!("{} {}\n", self.version, self.response.status);
        headers += &status_line;
        let content_length = self.response.body_length;
        self.response
            .set_header("content-length", &content_length.to_string());
        for (key, value) in &self.response.headers {
            let header = format!("{}: {}\n", key, value);
            headers += &header;
        }
        headers += "\n";
        self.raw.stream.write(headers.as_bytes()).await.unwrap();
        if content_length > 0 {
            self.raw
                .stream
                .write(&self.response.body_buffer[..content_length])
                .await
                .unwrap();
        }
        Ok(())
    }
}

impl<'a> Response<'a> {
    pub fn set_header(&mut self, header: &str, value: &str) -> Option<String> {
        self.headers.insert(
            header.to_string().trim().to_lowercase(),
            value.to_string().trim().to_string(),
        )
    }

    pub fn get_header(&mut self, header: &str) -> Option<&String> {
        self.headers.get(&header.to_string().trim().to_lowercase())
    }

    pub fn write_body(&mut self, bytes: &[u8]) -> Result<usize> {
        let body_buffer = &mut self.body_buffer[self.body_length..];
        let length = cmp::min(bytes.len(), body_buffer.len());
        body_buffer[..length].copy_from_slice(&bytes[..length]);
        self.body_length += length;
        if length != bytes.len() {
            return Err(Error::new(
                ErrorKind::FileTooLarge,
                "not all bytes were written",
            ));
        }
        Ok(length)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use indexmap::IndexMap;

    use tokio::io::AsyncReadExt;

    use crate::{HttpConn, RawHTTP, RawStream, Request, Response};

    fn new_response<'a>(
        headers: IndexMap<String, String>,
        status: usize,
        version: String,
        body: &'a mut [u8],
    ) -> HttpConn<'a, Cursor<Vec<u8>>> {
        let stream: Vec<u8> = Vec::new();
        let len = body.len();
        HttpConn {
            raw: RawHTTP {
                stream: RawStream::Normal(Cursor::new(stream)),
                buffer: &mut [],
                start: 0,
                end: 0,
            },
            version,
            request: Request {
                method: String::new(),
                uri: String::new(),
                headers: IndexMap::new(),
            },
            response: Response {
                status,
                headers,
                body_buffer: body,
                body_length: len,
            },
        }
    }

    #[tokio::test]
    async fn response_write_test() {
        let mut bodies = (
            Vec::from("hello world"),
            Vec::from("test2 2 2 2 2"),
            Vec::from("blaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n".to_owned())
        );
        let responses_in: Vec<HttpConn<Cursor<Vec<u8>>>> = vec![
            new_response(
                IndexMap::from([
                    ("server".to_string(), "jequi".to_string()),
                    ("content-type".to_string(), "application/json".to_string()),
                ]),
                301,
                "HTTP/1.1".to_string(),
                &mut bodies.0,
            ),
            new_response(
                IndexMap::from([
                    ("cache-control".to_string(), "max-age=1296000".to_string()),
                    (
                        "strict-transport-security".to_string(),
                        "max-age=31536000".to_string(),
                    ),
                ]),
                200,
                "HTTP/2".to_string(),
                &mut bodies.1,
            ),
            new_response(
                IndexMap::from([
                    ("content-length".to_string(), "1565".to_string()),
                    (
                        "set-cookie".to_string(),
                        "PHPSESSID=bla; path=/; domain=.example.com;HttpOnly;Secure;SameSite=None"
                            .to_string(),
                    ),
                ]),
                404,
                "HTTP/1".to_string(),
                &mut bodies.2,
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

            let buf = &mut [0; 1024];
            let mut n = 0;
            if let RawStream::Normal(mut stream) = r.raw.stream {
                stream.set_position(0);

                n = stream.read(buf).await.unwrap();
            }

            assert_eq!(
                String::from_utf8_lossy(&buf[..n]),
                String::from_utf8_lossy(expected_responses[i])
            )
        }
    }

    #[test]
    fn get_set_header_test() {
        let mut http = new_response(
            IndexMap::from([("server".to_string(), "not-jequi".to_string())]),
            0,
            String::new(),
            &mut [],
        );

        http.response.set_header("server", "jequi");
        let server = http.response.get_header("server");
        assert_eq!(Some(&"jequi".to_string()), server);

        http.response.set_header("Content-Length", "40");
        let server = http.response.get_header("content-length");
        assert_eq!(Some(&"40".to_string()), server);
    }

    #[tokio::test]
    async fn test_write_body() {
        let stream = Cursor::new(Vec::new());
        let mut body_buffer = [0; 1024];
        let mut http =
            HttpConn::new(RawStream::Normal(stream), &mut [0; 0], &mut body_buffer).await;
        let resp = &mut http.response;
        resp.write_body(b"hello").unwrap();
        assert_eq!(b"hello", &resp.body_buffer[..resp.body_length]);
        resp.write_body(b" world").unwrap();
        assert_eq!(b"hello world", &resp.body_buffer[..resp.body_length]);
    }
}
