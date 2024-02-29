use std::io::Result;
use std::task::Waker;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures::Future;

#[derive(Default, Debug)]
pub struct RequestBody {
    bytes: Option<Vec<u8>>,
    is_written: bool,
    waker: Option<Waker>,
}

unsafe impl Send for RequestBody {}

pub struct GetBody<'a> {
    body: *mut RequestBody,
    buf: &'a mut Vec<u8>,
}

unsafe impl Send for GetBody<'_> {}
unsafe impl Sync for GetBody<'_> {}

impl<'a> Future for GetBody<'a> {
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        println!("is it written?");
        if !unsafe { &*self.body }.is_written {
            unsafe { &mut *self.body }.waker = Some(cx.waker().clone());
            return Poll::Pending;
        }
        let bytes = &unsafe { &*self.body }.bytes;

        if let Some(bytes) = bytes.as_deref() {
            self.buf.extend_from_slice(bytes);
        }
        Poll::Ready(Ok(()))
    }
}

pub struct WriteBody<'a> {
    body: &'a mut RequestBody,
}

impl<'a> Future for WriteBody<'a> {
    type Output = Option<Vec<u8>>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if !self.body.is_written {
            return Poll::Pending;
        }
        Poll::Ready(self.body.bytes.clone())
    }
}

impl RequestBody {
    pub fn get_body(body: *mut RequestBody, buf: &mut Vec<u8>) -> GetBody {
        GetBody { body, buf }
    }

    pub fn write_body(&mut self, bytes: Option<Vec<u8>>) {
        self.bytes = bytes;
        self.is_written = true;
        if let Some(waker) = &self.waker {
            waker.wake_by_ref();
        }
    }
}
