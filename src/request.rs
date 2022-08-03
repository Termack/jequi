use std::io::{Read, ErrorKind};
use std::collections::HashMap;

struct RawRequest {
    stream: Box<dyn Read>,
    buffer: [u8;1024],
    start: usize,
    end: usize,
}

pub struct Request {
    raw: RawRequest,
    pub method: String,
    pub uri: String,
    pub version: String,
    pub headers: HashMap<String,String>,
}


impl Request {
    pub fn new(stream: Box<dyn Read>) -> Request{
        Request{
            raw:RawRequest{
                stream:stream,
                buffer: [0; 1024],
                start: 0,
                end: 0,
            },
            method:String::new(),
            uri:String::new(),
            version:String::new(),
            headers:HashMap::new()
        }
    }

    pub fn parse_first_line(&mut self) -> Result<(),&str> {
        struct StringIndex (Option<usize>,Option<usize>);
    
        let mut method_index = StringIndex(None,None);
        let mut uri_index = StringIndex(None,None);
        let mut version_index = StringIndex(None,None);

        let mut bytes = [0; 1024];
        match self.raw.stream.read(&mut self.raw.buffer) {
            Ok(0) => (),
            Ok(n) => {
                let mut get_next = true;
                bytes = self.raw.buffer;
                self.raw.end = n;
                for i in 0..n {
                    if bytes[i] == b'\n'{
                        if version_index.1 == None {
                            version_index.1 = Some(i);
                        }
                        self.raw.start = i;
                        break;
                    }
                    if !get_next && bytes[i] == b' '{
                        if method_index.1 == None {
                            method_index.1 = Some(i);
                            get_next = true;
                        }else if uri_index.1 == None{
                            uri_index.1 = Some(i);
                            get_next = true;
                        }else if version_index.1 == None {
                            version_index.1 = Some(i);
                            get_next = true;
                        }
                    }else if get_next && bytes[i] != b' ' {
                        if method_index.0 == None {
                            method_index.0 = Some(i);
                        }else if uri_index.0 == None {
                            if bytes[i] == b'/'{
                                uri_index.0 = Some(i);
                            }else{
                                panic!("Uri should start with /")
                            }
                        }else if version_index.0 == None {
                            version_index.0 = Some(i);
                        }else {
                            uri_index.1 = version_index.1;
                            version_index = StringIndex(Some(i),None);
                        }
                        get_next = false;
                    }
                }
            }
            Err(ref e) if e.kind() == ErrorKind::Interrupted => (),
            Err(e) => panic!("{:?}", e),
        }

        if version_index.1 == None {
            return Err("First line larger than buffer size");
        }

        let method_err = "Error getting method";
        let uri_err = "Error getting uri";
        let version_err = "Error getting version";
        
        self.method = String::from_utf8_lossy(
            &bytes[method_index.0.ok_or(method_err)?..method_index.1.ok_or(method_err)?]
        ).to_string();
        self.uri = String::from_utf8_lossy(
            &bytes[uri_index.0.ok_or(uri_err)?..uri_index.1.ok_or(uri_err)?]
        ).to_string();
        self.version = String::from_utf8_lossy(
            &bytes[version_index.0.ok_or(version_err)?..version_index.1.ok_or(version_err)?]
        ).to_string();
        Ok(())
    }

    pub fn parse_headers(&mut self) -> Result<(),&str> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn parse_first_line_test() {
        let requests_in: Vec<&[u8]> = vec![
            b"GET / HTTP/1.1 \n",
            b"POST /bla HTTP/2.0\n",
            b"PUT  /ab cd HTTP/1.2\n",
            b"  GET  / a adfsdab  HTTP/1.1 \n"
        ];

        #[derive(Debug)]
        #[derive(PartialEq)]
        struct Result {
            method: String,
            uri: String,
            version: String,
            index: usize
        }

        fn new_result(method: String,uri: String,version: String,index: usize) -> Result {
            Result{method,uri,version,index}
        }

        let expected_results = vec![
            new_result("GET".to_string(), "/".to_string(), "HTTP/1.1".to_string(),15),
            new_result("POST".to_string(), "/bla".to_string(), "HTTP/2.0".to_string(),18),
            new_result("PUT".to_string(), "/ab cd".to_string(), "HTTP/1.2".to_string(),20),
            new_result("GET".to_string(), "/ a adfsdab".to_string(), "HTTP/1.1".to_string(),29),
        ];

        for (i,r) in requests_in.iter().enumerate() {
            let mut req = Request::new(Box::new(*r));
            
            let err = req.parse_first_line();

            assert!(err.is_ok());

            assert_eq!(expected_results[i],new_result(req.method, req.uri, req.version, req.raw.start), "Testing parse for line: {}",String::from_utf8_lossy(r))
        }
    }

    fn parse_headers_test() {
        let requests_in: Vec<&[u8]> = vec![
            b"\
GET / HTTP/1.1 
Host: example.com
Content-Type: application/json
",
            b"\
POST /bla HTTP/2.0
User-Agent: Mozilla
Accept-Encoding: gzip
",
            b"\
PUT  /ab cd HTTP/1.2
Host: host.com
Cookies: aa=bb
",
            b"\
  GET  / a adfsdab  HTTP/1.1 \n
Bla: bla
Ble: ble
Header: haaa
Wowo: 10034mc amk
"
        ];

        let expected_results: Vec<HashMap<String,String>> = vec![
            HashMap::from([
                ("host".to_string(),"example.com".to_string()),
                ("content-type".to_string(),"application/json".to_string())]),
            HashMap::from([
                ("user-agent".to_string(),"Mozilla".to_string()),
                ("accept-encoding".to_string(),"gzip".to_string())]),
            HashMap::from([
                ("host".to_string(),"host.com".to_string()),
                ("cookies".to_string(),"aa=bb".to_string())]),
            HashMap::from([
                ("Bla".to_string(),"bla".to_string()),
                ("Ble".to_string(),"ble".to_string()),
                ("Header".to_string(),"haaa".to_string()),
                ("Wowo".to_string(),"10034mc amk".to_string())]),
        ];

        for (i,r) in requests_in.iter().enumerate() {
            let mut req = Request::new(Box::new(*r));
            
            let err = req.parse_headers();

            assert!(err.is_ok());

            assert_eq!(expected_results[i],req.headers, "Testing parse headers for request: {}",String::from_utf8_lossy(r))
        }
    }
}
