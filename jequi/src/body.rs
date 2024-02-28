use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures::Future;

use crate::RequestBody;

pub struct GetBody<'a> {
    body: &'a RequestBody,
}

impl<'a> Future for GetBody<'a> {
    type Output = Option<Vec<u8>>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if !*self.body.is_written {
            return Poll::Pending;
        }
        Poll::Ready(self.body.bytes.clone())
    }
}

pub struct WriteBody<'a> {
    body: &'a mut RequestBody,
}

impl<'a> Future for WriteBody<'a> {
    type Output = Option<Vec<u8>>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if !*self.body.is_written {
            return Poll::Pending;
        }
        Poll::Ready(self.body.bytes.clone())
    }
}

impl RequestBody {
    pub fn get_body(&self) -> GetBody {
        GetBody { body: self }
    }

    pub fn write_body(&mut self, bytes: Option<Vec<u8>>) {
        self.bytes = bytes;
        *self.is_written = true;
    }
}
