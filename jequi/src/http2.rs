use futures::{pin_mut, select, FutureExt};
use hpack_patched::{Decoder, Encoder};
use http::{HeaderMap, HeaderName, HeaderValue};
use std::{collections::HashMap, io::Cursor, sync::Arc};

use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufStream},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Mutex,
    },
};

use crate::{ConfigMap, HttpConn, RawStream, Request, Response};

#[derive(Debug)]
struct Http2Frame {
    length: u32,
    typ: u8,
    flags: u8,
    stream_id: u32,
    payload: Vec<u8>,
}

const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

const END_STREAM_FLAG: u8 = 0b00000001;
const END_HEADERS_FLAG: u8 = 0b00000100;
const PADDED_FLAG: u8 = 0b00001000;
const PRIORITY_FLAG: u8 = 0b00100000;

struct BufStreamRaw<T: AsyncRead + AsyncWrite + Unpin + Send>(*mut BufStream<T>);

unsafe impl<T: AsyncRead + AsyncWrite + Unpin + Send> Send for BufStreamRaw<T> {}
unsafe impl<T: AsyncRead + AsyncWrite + Unpin + Send> Sync for BufStreamRaw<T> {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send> BufStreamRaw<T> {
    pub fn get_mut(&mut self) -> &mut BufStream<T> {
        unsafe { &mut *self.0 }
    }
}

impl Http2Frame {
    async fn read_frame<T: AsyncRead + AsyncWrite + Unpin + Send>(
        mut stream: BufStreamRaw<T>,
    ) -> Self {
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
            typ,
            flags,
            stream_id,
            payload,
        }
    }

    fn encode(&self) -> Vec<u8> {
        let length = self.length;
        let mut buf = vec![0; 9 + length as usize];
        buf[0..3].copy_from_slice(&[
            ((length >> 16) & 0xff) as u8,
            ((length >> 8) & 0xff) as u8,
            (length & 0xff) as u8,
        ]);
        buf[3] = self.typ;
        buf[4] = self.flags;
        BigEndian::write_u32(&mut buf[5..], self.stream_id & ((1 << 31) - 1));
        buf[9..].copy_from_slice(&self.payload);
        buf
    }
}

struct Stream {
    id: u32,
    request: Arc<Request>,
    response: Arc<Response>,
}

pub struct Http2Conn<T: AsyncRead + AsyncWrite + Unpin + Send> {
    pub conn: BufStream<RawStream<T>>,
    streams: HashMap<u32, Arc<Stream>>,
}

impl<T: AsyncRead + AsyncWrite + Unpin + Send> From<HttpConn<T>> for Http2Conn<T> {
    fn from(http: HttpConn<T>) -> Self {
        Self {
            conn: http.conn,
            streams: HashMap::new(),
        }
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin + Send> Http2Conn<T> {
    pub async fn send_response(&mut self) {}
    pub async fn process_http2(&mut self, config_map: Arc<ConfigMap>) -> ! {
        let mut buf = vec![0; 24];
        self.conn.read_exact(&mut buf).await.unwrap();
        if buf != PREFACE {
            panic!("PREFACE WRONG");
        }
        self.conn.flush().await.unwrap();

        let raw = BufStreamRaw(&mut self.conn);
        println!("recv: {:?}", Http2Frame::read_frame(raw).await);
        let settings: &mut [u8] = &mut Http2Frame {
            length: 0,
            typ: 4,
            flags: 0,
            stream_id: 0,
            payload: Vec::new(),
        }
        .encode();
        self.conn.write_all(settings).await.unwrap();
        let ack: &mut [u8] = &mut Http2Frame {
            length: 0,
            typ: 4,
            flags: 1,
            stream_id: 0,
            payload: Vec::new(),
        }
        .encode();
        self.conn.write_all(ack).await.unwrap();
        self.conn.flush().await.unwrap();
        // println!(
        //     "sent: {:?}",
        //     Http2Frame::read_frame(&mut Cursor::new(ack)).await
        // );

        let mut decoder = Decoder::new();
        let mut encoder = Encoder::new();

        let (tx, mut rx): (Sender<u32>, Receiver<u32>) = channel(100);
        loop {
            let response_chan = rx.recv().fuse();
            let raw = BufStreamRaw(&mut self.conn);
            let read_fut = Http2Frame::read_frame(raw).fuse();
            pin_mut!(response_chan, read_fut);
            select! {
                stream_id = response_chan => {
                    let stream_id = stream_id.unwrap();
                    let stream = self.streams.remove(&stream_id).unwrap();
                    let response = stream.response.clone();
                    let compressed_headers = encoder.encode(
                        [(":status".as_bytes(), response.status.to_string().as_bytes())]
                            .into_iter()
                            .chain(
                                response
                                    .headers
                                    .iter()
                                    .map(|(h, v)| (h.as_ref(), v.as_bytes())),
                            ),
                    );
                    let response_headers = Http2Frame {
                        length: compressed_headers.len() as u32,
                        typ: 1,
                        flags: END_HEADERS_FLAG | END_STREAM_FLAG,
                        stream_id,
                        payload: compressed_headers,
                    };

                    println!("writing!");
                    self.conn
                        .write_all(&response_headers.encode())
                        .await
                        .unwrap();
                    self.conn.flush().await.unwrap();
                },
                frame = read_fut => {
                if frame.typ == 1 {
                    let flags = frame.flags;
                    let mut read_body = false;
                    let mut payload: &[u8] = &frame.payload;
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
                                    b":path" => {
                                        request.uri = String::from_utf8_lossy(v.as_ref()).to_string()
                                    }
                                    b":authority" => {
                                        request.host =
                                            Some(String::from_utf8_lossy(v.as_ref()).to_string())
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
                    let stream_id = frame.stream_id;
                    let mut request = Arc::new(request);
                    let mut response = Arc::new(Response::new());
                    let stream = Arc::new(Stream {
                        id: stream_id,
                        request: request.clone(),
                        response: response.clone(),
                    });
                    self.streams.insert(stream_id, stream.clone());

                    let config_map = config_map.clone();
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        let request = unsafe { Arc::get_mut_unchecked(&mut request) };
                        let response = unsafe { Arc::get_mut_unchecked(&mut response) };

                        if !read_body {
                            println!("noread");
                            request.body.get_mut().write_body(None);
                        }

                        request.handle_request(response, config_map).await;

                        if read_body {
                            println!("reading");
                            request.body.clone().get_body().await;
                        }

                        println!("noread");
                        tx.send(stream_id).await.unwrap();
                        println!("sent");
                    });
                }
                println!("recv: {:?}", frame)
                }
            }
        }
    }
}
