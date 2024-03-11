use hpack_patched::{Decoder, Encoder};
use http::{header, HeaderName};
use plugins::get_plugin;
use std::{collections::HashMap, sync::Arc};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufStream},
    sync::mpsc::{channel, Receiver, Sender},
};

use crate::{
    http2::{
        frame::{BufStreamRaw, FrameType},
        END_HEADERS_FLAG, PREFACE,
    },
    ConfigMap, HttpConn, RawStream,
};

use crate as jequi;

use super::{frame::Http2Frame, Stream, END_STREAM_FLAG};

const KEEP_ALIVE: HeaderName = HeaderName::from_static("keep-alive");

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
    async fn write_response(
        &mut self,
        stream_id: Option<u32>,
        encoder: &mut Encoder<'_>,
        config_map: Arc<ConfigMap>,
    ) {
        let stream_id = stream_id.unwrap();
        println!("response: {}", stream_id);
        let stream = self.streams.remove(&stream_id).unwrap();
        let (_, request, response) = Arc::into_inner(stream).unwrap().consume();
        let response = Arc::into_inner(response).unwrap();
        let compressed_headers = encoder.encode(
            [(":status".as_bytes(), response.status.to_string().as_bytes())]
                .into_iter()
                .chain(
                    response
                        .headers
                        .iter()
                        .filter(|(h, _)| {
                            !matches!(
                                **h,
                                header::TRANSFER_ENCODING | header::CONNECTION | KEEP_ALIVE
                            )
                        })
                        .map(|(h, v)| (h.as_ref(), v.as_bytes())),
                ),
        );

        let response_headers = Http2Frame::new(
            FrameType::Headers,
            END_HEADERS_FLAG,
            stream_id,
            compressed_headers,
        );

        let config = config_map.get_config_for_request(request.host.as_deref(), Some(&request.uri));
        let conf = get_plugin!(config, jequi);

        self.conn
            .write_all(&response_headers.encode())
            .await
            .unwrap();
        self.conn.flush().await.unwrap();

        let last = response.body_buffer.len().div_ceil(conf.chunk_size) - 1;
        for (i, chunk) in response.body_buffer.chunks(conf.chunk_size).enumerate() {
            let response_body = Http2Frame::new(
                FrameType::Data,
                if i == last { END_STREAM_FLAG } else { 0 },
                stream_id,
                chunk,
            );

            self.conn.write_all(&response_body.encode()).await.unwrap();
            self.conn.flush().await.unwrap();
        }
    }
    pub async fn process_http2(&mut self, config_map: Arc<ConfigMap>) -> ! {
        let mut buf = vec![0; 24];
        self.conn.read_exact(&mut buf).await.unwrap();
        if buf != PREFACE {
            panic!("PREFACE WRONG");
        }
        self.conn.flush().await.unwrap();

        let settings = Http2Frame::new(FrameType::Settings, 0, 0, Vec::new()).encode();
        self.conn.write_all(&settings).await.unwrap();

        let ack = Http2Frame::new(FrameType::Settings, 1, 0, Vec::new()).encode();
        self.conn.write_all(&ack).await.unwrap();
        self.conn.flush().await.unwrap();

        let mut decoder = Decoder::new();
        let mut encoder = Encoder::new();

        let (tx, mut rx): (Sender<u32>, Receiver<u32>) = channel(100);
        let raw = BufStreamRaw(&mut self.conn);
        let read_fut = Http2Frame::read_frame(raw);
        tokio::pin!(read_fut);
        loop {
            tokio::select! {
            frame = &mut read_fut => {
                frame.process_frame(&mut self.streams, &mut decoder, tx.clone(), config_map.clone()).await;
                let raw = BufStreamRaw(&mut self.conn);
                read_fut.set(Http2Frame::read_frame(raw));
            },
            stream_id = rx.recv() => self.write_response(stream_id, &mut encoder, config_map.clone()).await,
            }
        }
    }
}
