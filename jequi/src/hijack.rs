use crate::{http1::Http1Conn, AsyncRWSendBuf};

use futures::future::BoxFuture;

pub trait DynAsyncRWSend: AsyncRWSendBuf {}
impl<T: AsyncRWSendBuf> DynAsyncRWSend for T {}

pub enum PostRequestHandler {
    Continue,
    Exit,
    HijackConnection(
        Box<dyn FnOnce(Http1Conn<Box<dyn DynAsyncRWSend>>) -> BoxFuture<'static, ()> + Send>,
    ),
}
