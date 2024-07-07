use http::header;
use std::io::Result;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

use crate::AsyncRWSend;

use super::Http1Conn;

impl<'a, T: AsyncRWSend> Http1Conn<T> {
    pub async fn write_response(&mut self, chunk_size: usize) -> Result<()> {
        let mut headers = String::new();
        let status_line = format!("{} {}\n", self.version, self.response.status);
        headers += &status_line;
        let content_length = self.response.body_buffer.len();
        // TODO: add more checks to see if response should be chunked (like checking content-type)
        let chunked = content_length > chunk_size;
        if chunked {
            self.response.remove_header(header::CONTENT_LENGTH.as_str());
            self.response
                .set_header(header::TRANSFER_ENCODING.as_str(), "chunked");
        } else {
            self.response
                .remove_header(header::TRANSFER_ENCODING.as_str());
            self.response
                .set_header(header::CONTENT_LENGTH.as_str(), &content_length.to_string());
        }
        for (key, value) in &self.response.headers {
            let header = format!("{}: {}\n", key, value.to_str().unwrap());
            headers += &header;
        }
        headers += "\n";
        self.conn.write_all(headers.as_bytes()).await?;
        if chunked {
            for chunk in self.response.body_buffer.chunks(chunk_size) {
                let chunk = [
                    format!("{:x}\r\n", chunk.len()).as_bytes(),
                    &chunk,
                    "\r\n".as_bytes(),
                ]
                .concat();
                self.conn.write_all(&chunk).await?;
                self.conn.flush().await?;
            }
            self.conn.write_all("0\r\n\r\n".as_bytes()).await?;
            self.conn.flush().await?;
        } else {
            self.conn.write_all(&self.response.body_buffer).await?;
            self.conn.flush().await?;
        }
        Ok(())
    }
}
