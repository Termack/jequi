use std::borrow::Cow;
use std::io::{Error, ErrorKind, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};

use crate::{HttpConn, Request};

impl<'a, T: AsyncRead + AsyncWrite + Unpin> HttpConn<'a, T> {
    pub async fn parse_first_line(&mut self) -> Result<()> {
        struct StringIndex(Option<usize>, Option<usize>);

        let mut method_index = StringIndex(None, None);
        let mut uri_index = StringIndex(None, None);
        let mut version_index = StringIndex(None, None);

        match self.raw.stream.read(&mut self.raw.buffer).await {
            Ok(0) => (),
            Ok(n) => {
                let mut get_next = true;
                let bytes = &self.raw.buffer;
                self.raw.end = n;
                for i in 0..n {
                    if bytes[i] == b'\n' {
                        let mut index_end = i;
                        if bytes[i - 1] == b'\r' {
                            index_end = i - 1
                        }
                        if version_index.1 == None {
                            version_index.1 = Some(index_end);
                        }
                        self.raw.start = i + 1;
                        break;
                    }
                    if !get_next && bytes[i] == b' ' {
                        if method_index.1 == None {
                            method_index.1 = Some(i);
                            get_next = true;
                        } else if uri_index.1 == None {
                            uri_index.1 = Some(i);
                            get_next = true;
                        } else if version_index.1 == None {
                            version_index.1 = Some(i);
                            get_next = true;
                        }
                    } else if get_next && bytes[i] != b' ' {
                        if method_index.0 == None {
                            method_index.0 = Some(i);
                        } else if uri_index.0 == None {
                            if bytes[i] == b'/' {
                                uri_index.0 = Some(i);
                            } else {
                                panic!("Uri should start with /")
                            }
                        } else if version_index.0 == None {
                            version_index.0 = Some(i);
                        } else {
                            uri_index.1 = version_index.1;
                            version_index = StringIndex(Some(i), None);
                        }
                        get_next = false;
                    }
                }
            }
            Err(ref e) if e.kind() == ErrorKind::Interrupted => (),
            Err(e) => panic!("{:?}", e),
        }

        if version_index.1 == None {
            return Err(Error::new(
                ErrorKind::OutOfMemory,
                "First line larger than buffer size",
            ));
        }

        let method_err = "Error getting method";
        let uri_err = "Error getting uri";
        let version_err = "Error getting version";

        let method_index = (
            method_index
                .0
                .ok_or(Error::new(ErrorKind::Other, method_err)),
            method_index
                .1
                .ok_or(Error::new(ErrorKind::Other, method_err)),
        );
        let uri_index = (
            uri_index.0.ok_or(Error::new(ErrorKind::Other, uri_err)),
            uri_index.1.ok_or(Error::new(ErrorKind::Other, uri_err)),
        );
        let version_index = (
            version_index
                .0
                .ok_or(Error::new(ErrorKind::Other, version_err)),
            version_index
                .1
                .ok_or(Error::new(ErrorKind::Other, version_err)),
        );

        self.request.method =
            String::from_utf8_lossy(&self.raw.buffer[method_index.0?..method_index.1?]).to_string();
        self.request.uri =
            String::from_utf8_lossy(&self.raw.buffer[uri_index.0?..uri_index.1?]).to_string();
        self.version =
            String::from_utf8_lossy(&self.raw.buffer[version_index.0?..version_index.1?])
                .to_string();
        Ok(())
    }

    fn get_headers_from_bytes(&mut self) -> (bool, &[u8], Result<()>) {
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

            if buffer[i] == b'\n' {
                found_header = true;
                line_start = None;
                if let Some(ref header) = header {
                    let mut value: &[u8] = &[];
                    let mut value_end = i;
                    if let Some(prev) = buffer.get(i - 1) {
                        if *prev == b'\r' {
                            value_end = i - 1
                        }
                    }
                    if value_start != i {
                        value = &buffer[value_start..value_end];
                    }
                    let value = String::from_utf8_lossy(value).trim().to_string();
                    let header = header.trim().to_lowercase(); 
                    if header == "host" {
                        self.request.host = Some(value.clone());
                    }
                    self.request
                        .headers
                        .insert(header, value);
                } else {
                    return (
                        true,
                        &[],
                        Err(Error::new(ErrorKind::InvalidData, "Malformed header")),
                    );
                }
                if let Some(mut next) = buffer.get(i + 1) {
                    let mut index = i + 1;
                    if *next == b'\r' {
                        (next, index) = match buffer.get(i + 2) {
                            Some(next) => (next, i + 2),
                            None => (next, index),
                        };
                    }
                    if *next == b'\n' {
                        stop = true;
                        line_start = Some(index+1);
                        break;
                    }
                }
            } else if buffer[i] == b':' {
                header = Some(String::from_utf8_lossy(&buffer[line_start.unwrap()..i]));
                value_start = i + 1
            }
        }

        let mut start = 0;
        if let Some(index) = line_start {
            start = index
        }

        let buf = &buffer[start..buffer.len()];

        let mut res = Ok(());

        if !found_header {
            res = Err(Error::new(ErrorKind::FileTooLarge, "Header too big"));
        }

        (stop, buf, res)
    }

    pub async fn parse_headers(&mut self) -> Result<()> {
        if self.raw.end != 0 {
            let (stop, buffer, err) = self.get_headers_from_bytes();
            if let Err(err) = err {
                if err.kind() == ErrorKind::InvalidData {
                    return Err(err);
                }
            }
            let buf = Vec::from(buffer);
            self.raw.buffer[..buf.len()].copy_from_slice(&buf);
            if stop {
                self.raw.start = 0;
                self.raw.end = buf.len();
                return Ok(());
            }
            self.raw.start = buf.len();
        }
        loop {
            match self
                .raw
                .stream
                .read(&mut self.raw.buffer[self.raw.start..])
                .await
            {
                Ok(0) => {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        "Request headers in wrong format",
                    ))
                }
                Ok(n) => {
                    self.raw.end = self.raw.start + n;
                    self.raw.start = 0;
                    let (stop, buffer, err) = self.get_headers_from_bytes();
                    if let Err(err) = err {
                        return Err(err);
                    }
                    let buf = Vec::from(buffer);
                    self.raw.buffer[..buf.len()].copy_from_slice(&buf);
                    if stop {
                        self.raw.start = 0;
                        self.raw.end = buf.len();
                        return Ok(());
                    }
                    self.raw.start = buf.len();
                }
                Err(ref e) if e.kind() == ErrorKind::Interrupted => (),
                Err(e) => panic!("{:?}", e),
            }
        }
    }

    pub async fn read_body(&mut self) -> Result<()> {
        let content_length: usize = self
            .request
            .get_header("Content-Length")
            .ok_or(Error::new(ErrorKind::NotFound, "No content length"))?
            .parse()
            .map_err(|err| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("Cant convert content length to int: {}", err),
                )
            })?;
        let mut body: Vec<u8> = Vec::with_capacity(content_length);
        if self.raw.end != 0 {
            let mut end = self.raw.end;
            let start = self.raw.start;
            if self.raw.end - self.raw.start > content_length {
                end = self.raw.start + content_length;
                self.raw.start = end;
            }
            body.extend_from_slice(&self.raw.buffer[start..end]);
            if body.len() == content_length {
                self.request.body = Some(String::from_utf8_lossy(&body).into_owned());
                return Ok(());
            }
        }
        self.raw.start = 0;
        loop {
            match self.raw.stream.read(&mut self.raw.buffer).await {
                Ok(0) => {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        "Content length and body size dont match",
                    ))
                }
                Ok(n) => {
                    self.raw.end = n;
                    let mut end = self.raw.end;
                    let start = self.raw.start;
                    if self.raw.end - self.raw.start > content_length {
                        end = self.raw.start + content_length;
                        self.raw.start = end;
                    }
                    body.extend_from_slice(&self.raw.buffer[start..end]);
                    if body.len() == content_length {
                        self.request.body = Some(String::from_utf8_lossy(&body).into_owned());
                        return Ok(());
                    }
                }
                Err(ref e) if e.kind() == ErrorKind::Interrupted => (),
                Err(e) => panic!("{:?}", e),
            }
        }
    }
}

impl<'a> Request {
    pub fn get_header(&self, header: &str) -> Option<&String> {
        self.headers.get(&header.to_string().trim().to_lowercase())
    }

    pub fn get_body(&self) -> Option<&String> {
        self.body.as_ref()
    }
}

#[cfg(test)]
mod test;
