use crate::{http1::Http1Conn, AsyncRWSend};

pub trait DynAsyncRWSend: AsyncRWSend {}
impl<T: AsyncRWSend> DynAsyncRWSend for T {}

pub enum PostRequestHandler {
    Continue,
    Exit,
    HijackConnection(Box<dyn Fn(Http1Conn<Box<dyn DynAsyncRWSend>>) + Send>),
}
