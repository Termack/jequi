use std::sync::Arc;

use crate::{Request, Response};

pub mod conn;
pub mod frame;

const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

const END_STREAM_FLAG: u8 = 0b00000001;
const END_HEADERS_FLAG: u8 = 0b00000100;
const PADDED_FLAG: u8 = 0b00001000;
const PRIORITY_FLAG: u8 = 0b00100000;

pub(crate) struct Stream {
    id: u32,
    request: Arc<Request>,
    response: Arc<Response>,
}

impl Stream {
    fn consume(self) -> (u32, Arc<Request>, Arc<Response>) {
        (self.id, self.request, self.response)
    }
}
