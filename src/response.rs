use std::io::{Read, Write, Result};

use crate::{HttpConn, Response};

impl<'a, T: Read + Write> HttpConn<'a, T> {
    pub fn write_response(&mut self) -> Result<()> {
        let status_line = format!("{} {}\n",self.version,self.response.status);
        self.raw.stream.write(status_line.as_bytes()).unwrap();
        for (key, value) in &self.response.headers {
            let header = format!("{}: {}\n",key,value);
            self.raw.stream.write(header.as_bytes()).unwrap();
        }
        self.raw.stream.write(b"\n").unwrap();
        Ok(())
    }
}

impl Response {
    pub fn set_header(&mut self, header: &str, value: &str ) -> Option<String> {
        self.headers.insert(header.to_string().trim().to_lowercase(), value.to_string().trim().to_string())
    }

    pub fn get_header(&mut self, header: &str) -> Option<&String> {
        self.headers.get(&header.to_string().trim().to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Cursor};

    use indexmap::IndexMap;

    use crate::{HttpConn, RawHTTP, Request, Response, RawStream};

    fn new_response(headers: IndexMap<String,String>, status: usize, version: String) -> HttpConn<'static, Cursor<Vec<u8>>> {
        let stream: Vec<u8> = Vec::new();
        static mut BUFFER: &mut [u8] = &mut [0;1];
        HttpConn{
            raw:RawHTTP{
                stream: RawStream::Normal(Cursor::new(stream)),
                buffer: unsafe{BUFFER},
                start: 0,
                end: 0,
            },
            version,
            request: Request { 
                method:String::new(),
                uri:String::new(),
                headers:IndexMap::new() 
            },
            response: Response {
                status,
                headers
            }
        }
    }

    #[test]
    fn response_write_test() {
        let responses_in: Vec<HttpConn<Cursor<Vec<u8>>>> = vec![
            new_response(
                IndexMap::from([
                    ("server".to_string(),"jequi".to_string()),
                    ("content-type".to_string(),"application/json".to_string())]
                ),
                301,
                "HTTP/1.1".to_string()
            ),
            new_response(
                IndexMap::from([
                    ("cache-control".to_string(),"max-age=1296000".to_string()),
                    ("strict-transport-security".to_string(),"max-age=31536000".to_string())]
                ),
                200,
                "HTTP/2".to_string()
            ),
            new_response(
                IndexMap::from([
                    ("content-length".to_string(),"1565".to_string()),
                    ("set-cookie".to_string(),"PHPSESSID=bla; path=/; domain=.example.com;HttpOnly;Secure;SameSite=None".to_string())]
                ),
                404,
                "HTTP/1".to_string()
            )
        ];

        let expected_responses: Vec<&[u8]> = vec![
            b"\
HTTP/1.1 301
server: jequi
content-type: application/json

",
            b"\
HTTP/2 200
cache-control: max-age=1296000
strict-transport-security: max-age=31536000

",
            b"\
HTTP/1 404
content-length: 1565
set-cookie: PHPSESSID=bla; path=/; domain=.example.com;HttpOnly;Secure;SameSite=None

",
        ];
        for (i, mut r) in responses_in.into_iter().enumerate() {
            r.write_response().unwrap();

            let buf = &mut [0;1024];
            let mut n = 0;
            if let RawStream::Normal(mut stream) = r.raw.stream {
                stream.set_position(0);
                
                n = stream.read(buf).unwrap();
            }

            assert_eq!(String::from_utf8_lossy(&buf[..n]),String::from_utf8_lossy(expected_responses[i]))
        }
    }

    #[test]
    fn get_set_header_test() {
        let mut http = new_response(
        IndexMap::from([
            ("server".to_string(),"not-jequi".to_string())
        ]),
        0,
        String::new());

        http.response.set_header("server","jequi");
        let server = http.response.get_header("server");
        assert_eq!(Some(&"jequi".to_string()),server);

        http.response.set_header("Content-Length","40");
        let server = http.response.get_header("content-length");
        assert_eq!(Some(&"40".to_string()),server);
    }
}