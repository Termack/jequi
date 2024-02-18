use std::io::Cursor;

use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::HttpConn;

#[derive(Debug)]
struct Http2Frame {
    length: u32,
    typ: u8,
    flags: u8,
    stream_id: u32,
    payload: Vec<u8>,
}

const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

impl Http2Frame {
    async fn read_frame<T: AsyncReadExt + Unpin>(stream: &mut T) -> Self {
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

pub async fn process_http2<T: AsyncRead + AsyncWrite + Unpin>(mut http: HttpConn<T>) {
    let mut buf = vec![0; 24];
    http.stream.read_exact(&mut buf).await.unwrap();
    if buf != PREFACE {
        panic!("PREFACE WRONG");
    }
    http.stream.flush().await.unwrap();

    println!("recv: {:?}", Http2Frame::read_frame(&mut http.stream).await);
    let settings: &mut [u8] = &mut Http2Frame {
        length: 0,
        typ: 4,
        flags: 0,
        stream_id: 0,
        payload: Vec::new(),
    }
    .encode();
    http.stream.write_all(settings).await.unwrap();
    let ack: &mut [u8] = &mut Http2Frame {
        length: 0,
        typ: 4,
        flags: 1,
        stream_id: 0,
        payload: Vec::new(),
    }
    .encode();
    http.stream.write_all(ack).await.unwrap();
    http.stream.flush().await.unwrap();
    println!(
        "sent: {:?}",
        Http2Frame::read_frame(&mut Cursor::new(ack)).await
    );

    loop {
        println!("recv: {:?}", Http2Frame::read_frame(&mut http.stream).await)
    }
}
