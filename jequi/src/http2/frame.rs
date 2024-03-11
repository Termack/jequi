use byteorder::{BigEndian, ByteOrder};
use hpack_patched::Decoder;
use http::{HeaderName, HeaderValue};
use std::{
    collections::HashMap,
    io::{Error, ErrorKind, Result},
    sync::Arc,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, BufStream},
    sync::mpsc::Sender,
};

use crate::{ConfigMap, Request, Response};

use super::{Stream, END_STREAM_FLAG, PADDED_FLAG, PRIORITY_FLAG};

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

impl From<&FrameType> for u8 {
    fn from(val: &FrameType) -> Self {
        match val {
            FrameType::Data => 0,
            FrameType::Headers => 1,
            FrameType::Priority => 2,
            FrameType::RstStream => 3,
            FrameType::Settings => 4,
            FrameType::PushPromise => 5,
            FrameType::Ping => 6,
            FrameType::GoAway => 7,
            FrameType::WindowUpdate => 8,
            FrameType::Continuation => 9,
        }
    }
}

impl TryFrom<u8> for FrameType {
    type Error = Error;

    fn try_from(value: u8) -> Result<FrameType> {
        match value {
            0 => Ok(FrameType::Data),
            1 => Ok(FrameType::Headers),
            2 => Ok(FrameType::Priority),
            3 => Ok(FrameType::RstStream),
            4 => Ok(FrameType::Settings),
            5 => Ok(FrameType::PushPromise),
            6 => Ok(FrameType::Ping),
            7 => Ok(FrameType::GoAway),
            8 => Ok(FrameType::WindowUpdate),
            9 => Ok(FrameType::Continuation),
            _ => Err(Error::new(ErrorKind::InvalidData, "invalid data")),
        }
    }
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

pub struct BufStreamRaw<T: AsyncRead + AsyncWrite + Unpin + Send>(pub *mut BufStream<T>);

unsafe impl<T: AsyncRead + AsyncWrite + Unpin + Send> Send for BufStreamRaw<T> {}
unsafe impl<T: AsyncRead + AsyncWrite + Unpin + Send> Sync for BufStreamRaw<T> {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send> BufStreamRaw<T> {
    pub fn get_mut(&mut self) -> &mut BufStream<T> {
        unsafe { &mut *self.0 }
    }
}

impl Http2Frame<Vec<u8>> {
    pub async fn read_frame<T: AsyncRead + AsyncWrite + Unpin + Send>(
        mut stream: BufStreamRaw<T>,
    ) -> Http2Frame<Vec<u8>> {
        let stream = stream.get_mut();
        let mut buf = vec![0; 9];
        stream.read_exact(&mut buf).await.unwrap();

        let length = ((buf[0] as u32) << 16) + ((buf[1] as u32) << 8) + buf[2] as u32;
        let typ = buf[3];
        let flags = buf[4];
        let stream_id = BigEndian::read_u32(&buf[5..]) & ((1 << 31) - 1);

        let mut payload = vec![0; length as usize];
        stream.read_exact(&mut payload).await.unwrap();

        Self {
            length,
            typ: typ.try_into().unwrap(),
            flags,
            stream_id,
            payload,
        }
    }

    pub(crate) async fn process_frame(
        self,
        streams: &mut HashMap<u32, Arc<Stream>>,
        decoder: &mut Decoder<'_>,
        tx: Sender<u32>,
        config_map: Arc<ConfigMap>,
    ) {
        println!("recv: {:?}", self);
        match self.typ {
            FrameType::Data => self.process_data(streams),
            FrameType::Headers => self.process_headers(streams, decoder, tx, config_map).await,
            _ => (),
        };
    }

    fn process_data(self, streams: &HashMap<u32, Arc<Stream>>) {
        let mut request = streams.get(&self.stream_id).unwrap().request.clone();
        unsafe { Arc::get_mut_unchecked(&mut request) }
            .body
            .get_mut()
            .write_body(Some(self.payload));
    }
}

impl<P: AsRef<[u8]>> Http2Frame<P> {
    pub fn new(typ: FrameType, flags: u8, stream_id: u32, payload: P) -> Http2Frame<P> {
        Http2Frame {
            length: payload.as_ref().len() as u32,
            typ,
            flags,
            stream_id,
            payload,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let length = self.length;
        let mut buf = vec![0; 9 + length as usize];
        buf[0..3].copy_from_slice(&[
            ((length >> 16) & 0xff) as u8,
            ((length >> 8) & 0xff) as u8,
            (length & 0xff) as u8,
        ]);
        buf[3] = (&self.typ).into();
        buf[4] = self.flags;
        BigEndian::write_u32(&mut buf[5..], self.stream_id & ((1 << 31) - 1));
        buf[9..].copy_from_slice(self.payload.as_ref());
        buf
    }

    async fn process_headers(
        self,
        streams: &mut HashMap<u32, Arc<Stream>>,
        decoder: &mut Decoder<'_>,
        tx: Sender<u32>,
        config_map: Arc<ConfigMap>,
    ) {
        let flags = self.flags;
        let mut read_body = false;
        let mut payload: &[u8] = self.payload.as_ref();
        if flags & PADDED_FLAG != 0 {
            let _padding_length = payload.read_u8().await.unwrap();
        }
        if flags & PRIORITY_FLAG != 0 {
            payload.read_exact(&mut [0; 5]).await.unwrap();
        }
        if flags & END_STREAM_FLAG == 0 {
            read_body = true;
        }

        let mut request = Request::new();

        decoder
            .decode_with_cb(payload, |h, v| {
                if h[0] == b':' {
                    match h.as_ref() {
                        b":method" => {
                            request.method = String::from_utf8_lossy(v.as_ref()).to_string()
                        }
                        b":path" => request.uri = String::from_utf8_lossy(v.as_ref()).to_string(),
                        b":authority" => {
                            request.host = Some(String::from_utf8_lossy(v.as_ref()).to_string())
                        }
                        _ => (),
                    }
                    return;
                }
                request.headers.append(
                    HeaderName::from_bytes(h.as_ref()).unwrap(),
                    HeaderValue::from_bytes(v.as_ref()).unwrap(),
                );
            })
            .unwrap();
        let stream_id = self.stream_id;
        let mut request = Arc::new(request);
        let mut response = Arc::new(Response::new());
        let stream = Arc::new(Stream {
            id: stream_id,
            request: request.clone(),
            response: response.clone(),
        });
        streams.insert(stream_id, stream.clone());

        tokio::spawn(async move {
            let request = unsafe { Arc::get_mut_unchecked(&mut request) };
            let response = unsafe { Arc::get_mut_unchecked(&mut response) };

            if !read_body {
                request.body.get_mut().write_body(None);
            }

            request.handle_request(response, config_map).await;

            if read_body {
                request.body.clone().get_body().await;
            }

            println!("processed: {}", stream_id);
            tx.send(stream_id).await.unwrap();
        });
    }
}
