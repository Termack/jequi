use std::io::{Read, ErrorKind,Result,Error, Write};
use std::borrow::Cow;

use crate::HttpConn;

impl<'a, T: Read + Write> HttpConn<'a, T> {
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
                        let mut index_end = i;
                        if bytes[i-1] == b'\r'{
                            index_end = i-1
                        }
                        if version_index.1 == None {
                            version_index.1 = Some(index_end);
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


        self.request.method = String::from_utf8_lossy(
            &self.raw.buffer[method_index.0?..method_index.1?]
        ).to_string();
        self.request.uri = String::from_utf8_lossy(
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
                    let mut value_end = i;
                    if let Some(prev) = buffer.get(i-1){
                        if *prev == b'\r' {
                            value_end = i-1
                        }
                    }
                    if value_start != i {
                        value = &buffer[value_start..value_end];
                    }
                    let value = String::from_utf8_lossy(value).to_string();
                    self.request.headers.insert(header.trim().to_lowercase(), value.trim().to_string());
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

    use std::io::Cursor;

    use indexmap::IndexMap;

    use crate::RawStream;

    use super::*;

    #[test]
    fn parse_first_line_test() {
        let requests_in: Vec<Vec<u8>> = vec![
            Vec::from("GET / HTTP/1.1 \n"),
            Vec::from("POST /bla HTTP/2.0\n"),
            Vec::from("PUT  /ab cd HTTP/1.2\n"),
            Vec::from("  GET  / a adfsdab  HTTP/1.1 \n")
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
            let mut req = HttpConn::new(RawStream::Normal(Cursor::new(r.clone())),&mut buf);
            
            let err = req.parse_first_line();

            err.unwrap();

            assert_eq!(expected_results[i],new_result(req.request.method, req.request.uri, req.version, req.raw.start), "Testing parse for line: {}",String::from_utf8_lossy(r))
        }
    }

    #[test]
    fn parse_headers_test() {
        let requests_in: Vec<Vec<u8>> = vec![
            Vec::from("\
GET / HTTP/1.1 
Host: example.com
Content-Type: application/json

"),
            Vec::from("\
POST /bla HTTP/2.0
User-Agent: Mozilla
Accept-Encoding: gzip

"),
            Vec::from("\
PUT  /ab cd HTTP/1.2
Host: host.com
Cookies: aa=bb

"),
            Vec::from("\
  GET  / a adfsdab  HTTP/1.1 
Bla: bla
Ble: ble
Header: haaa
Wowo: 10034mc amk

")
        ];

        let expected_results: Vec<IndexMap<String,String>> = vec![
            IndexMap::from([
                ("host".to_string(),"example.com".to_string()),
                ("content-type".to_string(),"application/json".to_string())]),
            IndexMap::from([
                ("user-agent".to_string(),"Mozilla".to_string()),
                ("accept-encoding".to_string(),"gzip".to_string())]),
            IndexMap::from([
                ("host".to_string(),"host.com".to_string()),
                ("cookies".to_string(),"aa=bb".to_string())]),
            IndexMap::from([
                ("bla".to_string(),"bla".to_string()),
                ("ble".to_string(),"ble".to_string()),
                ("header".to_string(),"haaa".to_string()),
                ("wowo".to_string(),"10034mc amk".to_string())]),
        ];

        for (i,r) in requests_in.iter().enumerate() {
            let mut buf = [0; 35];
            let mut req = HttpConn::new(RawStream::Normal(Cursor::new(r.clone())),&mut buf);

            req.parse_first_line().unwrap();
            
            req.parse_headers().unwrap();

            assert_eq!(expected_results[i],req.request.headers, "Testing parse headers for request: {}",String::from_utf8_lossy(r))
        }
    }
}
