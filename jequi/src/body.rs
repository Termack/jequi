use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use std::task::Waker;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures::Future;

#[derive(Default, Debug)]
pub struct RequestBody {
    bytes: Arc<Option<Vec<u8>>>,
    is_written: AtomicBool,
    waker: Option<Waker>,
}

pub struct GetBody {
    body: Arc<RequestBody>,
}

unsafe impl Send for GetBody {}
unsafe impl Sync for GetBody {}

impl Future for GetBody {
    type Output = Arc<Option<Vec<u8>>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if !&self.body.is_written.load(Relaxed) {
            self.body.get_mut().waker = Some(cx.waker().clone());
            return Poll::Pending;
        }

        Poll::Ready(self.body.clone().bytes.clone())
    }
}

impl<'a> RequestBody {
    pub fn get_body(self: Arc<Self>) -> GetBody {
        GetBody { body: self }
    }

    pub fn get_mut(self: &'a mut Arc<Self>) -> &'a mut Self {
        unsafe { Arc::get_mut_unchecked(self) }
    }

    pub fn write_body(&mut self, bytes: Option<Vec<u8>>) {
        self.bytes = Arc::new(bytes);
        self.is_written.store(true, Relaxed);
        if let Some(waker) = &self.waker {
            waker.wake_by_ref();
        }
    }
}
