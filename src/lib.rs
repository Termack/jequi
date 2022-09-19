#![feature(io_error_more)]
pub mod request;
pub mod ssl;

use std::{collections::HashMap, io::{Result,Read, Write}};

use openssl::ssl::SslStream;

pub enum RawStream<T: Read + Write> {
    Ssl(SslStream<T>),
    Normal(T)
}

impl<S: Read + Write> Read for RawStream<S>
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match *self {
            RawStream::Ssl(ref mut s) => s.read(buf),
            RawStream::Normal(ref mut s) => s.read(buf),
        }
    }
}

impl<S: Read + Write> Write for RawStream<S>
{
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        match *self {
            RawStream::Ssl(ref mut s) => s.write(buf),
            RawStream::Normal(ref mut s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> Result<()> {
        match *self {
            RawStream::Ssl(ref mut s) => s.flush(),
            RawStream::Normal(ref mut s) => s.flush(),
        }
    }
}

pub struct RawHTTP<'a, T: Read + Write> {
    pub stream: RawStream<T>,
    pub buffer: &'a mut [u8],
    pub start: usize,
    pub end: usize,
}

pub struct Request {
    pub method: String,
    pub uri: String,
    pub headers: HashMap<String,String>,
}

pub struct Response {
    pub status: usize,
    pub headers: HashMap<String,String>,
}

pub struct HttpConn<'a, T: Read + Write> {
    pub raw: RawHTTP<'a, T>,
    pub version: String,
    pub request: Request,
    pub response: Response,
}

impl<'a, T: Read + Write> HttpConn<'a, T> {
    pub fn new(stream: RawStream<T>,buffer: &mut [u8]) -> HttpConn<T>{
        HttpConn{
            raw:RawHTTP{
                stream,
                buffer,
                start: 0,
                end: 0,
            },
            version:String::new(),
            request: Request { 
                method:String::new(),
                uri:String::new(),
                headers:HashMap::new() 
            },
            response: Response {
                status: 0,
                headers: HashMap::new()
            }
        }
    }
}