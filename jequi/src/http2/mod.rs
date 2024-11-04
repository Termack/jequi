use derivative::Derivative;
use std::{collections::HashMap, sync::Arc};

use crate::{AsyncRWSendBuf, Request, Response};

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

#[derive(Derivative)]
#[derivative(Default)]
pub(crate) struct Settings {
    #[derivative(Default(value = "16_384"))]
    max_frame_size: u32,
}

pub struct Http2Conn<T: AsyncRWSendBuf> {
    pub conn: T,
    settings: Settings,
    streams: HashMap<u32, Arc<Stream>>,
}

#[derive(Debug)]
pub struct Http2Frame<P>
where
    P: AsRef<[u8]>,
{
    length: u32,
    typ: FrameType,
    flags: u8,
    stream_id: u32,
    payload: P,
}

#[derive(Debug)]
pub enum FrameType {
    Data,
    Headers,
    Priority,
    RstStream,
    Settings,
    PushPromise,
    Ping,
    GoAway,
    WindowUpdate,
    Continuation,
}

pub struct BufStreamRaw<T: AsyncRWSendBuf>(pub *mut T);

impl Stream {
    fn consume(self) -> (u32, Arc<Request>, Arc<Response>) {
        (self.id, self.request, self.response)
    }
}

#[cfg(test)]
mod test;
