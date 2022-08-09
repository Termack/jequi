use std::io::{Read, ErrorKind,Result,Error};
use std::collections::HashMap;
use std::borrow::Cow;

pub struct RawRequest<'a, T: Read> {
    pub stream: Box<T>,
    pub buffer: &'a mut [u8],
    pub start: usize,
    pub end: usize,
}

pub struct Request<'a, T: Read> {
    pub raw: RawRequest<'a, T>,
    pub method: String,
    pub uri: String,
    pub version: String,
    pub headers: HashMap<String,String>,
}


impl<'a, T: Read> Request<'a, T> {
    pub fn new(stream: Box<T>,buffer: &mut [u8]) -> Request<T>{
        Request{
            raw:RawRequest{
                stream,
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

    pub fn parse_first_line(&mut self) -> Result<()> {
        struct StringIndex (Option<usize>,Option<usize>);
    
        let mut method_index = StringIndex(None,None);
        let mut uri_index = StringIndex(None,None);
        let mut version_index = StringIndex(None,None);

        match self.raw.stream.read(&mut self.raw.buffer) {
            Ok(0) => (),
            Ok(n) => {
                let mut get_next = true;
                let bytes = &self.raw.buffer;
                self.raw.end = n;
                for i in 0..n {
                    if bytes[i] == b'\n'{
                        if version_index.1 == None {
                            version_index.1 = Some(i);
                        }
                        self.raw.start = i+1;
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
            return Err(Error::new(ErrorKind::OutOfMemory,"First line larger than buffer size"));
        }

        let method_err = "Error getting method";
        let uri_err = "Error getting uri";
        let version_err = "Error getting version";

        let method_index = (method_index.0.ok_or(Error::new(ErrorKind::Other,method_err)),method_index.1.ok_or(Error::new(ErrorKind::Other,method_err)));
        let uri_index = (uri_index.0.ok_or(Error::new(ErrorKind::Other,uri_err)),uri_index.1.ok_or(Error::new(ErrorKind::Other,uri_err)));
        let version_index = (version_index.0.ok_or(Error::new(ErrorKind::Other,version_err)),version_index.1.ok_or(Error::new(ErrorKind::Other,version_err)));


        self.method = String::from_utf8_lossy(
            &self.raw.buffer[method_index.0?..method_index.1?]
        ).to_string();
        self.uri = String::from_utf8_lossy(
            &self.raw.buffer[uri_index.0?..uri_index.1?]
        ).to_string();
        self.version = String::from_utf8_lossy(
            &self.raw.buffer[version_index.0?..version_index.1?]
        ).to_string();
        Ok(())
    }

    fn get_headers_from_bytes(&mut self) -> (bool,&[u8],Result<()>){
        let mut line_start = None;
        let mut stop = false;
        let mut header: Option<Cow<str>> = None;
        let mut value_start = 0;
        let mut found_header = false;
        let buffer = &self.raw.buffer[self.raw.start..self.raw.end];
        for i in 0..buffer.len() {
            if line_start == None {
                line_start = Some(i);
            }

            if buffer[i] == b'\n'{
                found_header = true;
                line_start = None;
                if let Some(ref header) = header {
                    let mut value: &[u8] = &[];
                    if value_start != i {
                        value = &buffer[value_start..i];
                    }
                    let value = String::from_utf8_lossy(value).to_string();
                    self.headers.insert(header.trim().to_lowercase(), value.trim().to_string());
                }else{
                    return (true,&[],Err(Error::new(ErrorKind::InvalidData,"Malformed header")));
                }
                if let Some(next) = buffer.get(i+1){
                    if *next == b'\n' {
                        stop = true;
                        break;
                    }
                }
            }else if buffer[i] == b':'{
                header = Some(String::from_utf8_lossy(&buffer[line_start.unwrap()..i]));
                value_start = i+1
            }

        }

        let mut buf: &[u8] = &[];

        if let Some(index) = line_start {
            buf = &buffer[index..buffer.len()]
        }

        let mut res = Ok(());

        if !found_header {
            res = Err(Error::new(ErrorKind::FileTooLarge,"Header too big"));
        }

        (stop,buf,res)
    }

    pub fn parse_headers(&mut self) -> Result<()> {
        if self.raw.end != 0 {
            let (stop, buffer, err) = self.get_headers_from_bytes();
            if let Err(err) = err {
                if err.kind() == ErrorKind::InvalidData {
                    return Err(err);
                }
            }
            if stop {
                return Ok(());
            }
            let buf = Vec::from(buffer);
            self.raw.buffer[..buf.len()].copy_from_slice(&buf);
            self.raw.start = buf.len();
        }
        loop {
            match self.raw.stream.read(&mut self.raw.buffer[self.raw.start..]) {
                Ok(0) => return Err(Error::new(ErrorKind::Other,"Something went wrong")),
                Ok(n) => {
                    self.raw.end = self.raw.start+n;
                    self.raw.start = 0;
                    let (stop, buffer, err) = self.get_headers_from_bytes();
                    if let Err(err) = err {
                        return Err(err);
                    }
                    if stop {
                        return Ok(());
                    }
                    let buf = Vec::from(buffer);
                    self.raw.buffer[..buf.len()].copy_from_slice(&buf);
                    self.raw.start = buf.len();
                }
                Err(ref e) if e.kind() == ErrorKind::Interrupted => (),
                Err(e) => panic!("{:?}", e),
            }
        }
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
            new_result("GET".to_string(), "/".to_string(), "HTTP/1.1".to_string(),16),
            new_result("POST".to_string(), "/bla".to_string(), "HTTP/2.0".to_string(),19),
            new_result("PUT".to_string(), "/ab cd".to_string(), "HTTP/1.2".to_string(),21),
            new_result("GET".to_string(), "/ a adfsdab".to_string(), "HTTP/1.1".to_string(),30),
        ];

        for (i,r) in requests_in.iter().enumerate() {
            let mut buf = [0;35];
            let mut req = Request::new(Box::new(*r),&mut buf);
            
            let err = req.parse_first_line();

            err.unwrap();

            assert_eq!(expected_results[i],new_result(req.method, req.uri, req.version, req.raw.start), "Testing parse for line: {}",String::from_utf8_lossy(r))
        }
    }

    #[test]
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
  GET  / a adfsdab  HTTP/1.1 
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
                ("bla".to_string(),"bla".to_string()),
                ("ble".to_string(),"ble".to_string()),
                ("header".to_string(),"haaa".to_string()),
                ("wowo".to_string(),"10034mc amk".to_string())]),
        ];

        for (i,r) in requests_in.iter().enumerate() {
            let mut buf = [0; 35];
            let mut req = Request::new(Box::new(*r),&mut buf);

            let err = req.parse_first_line();

            err.unwrap();
            
            let err = req.parse_headers();

            err.unwrap();

            assert_eq!(expected_results[i],req.headers, "Testing parse headers for request: {}",String::from_utf8_lossy(r))
        }
    }
}
