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
        match self.body.clone().try_get_body() {
            Some(body) => Poll::Ready(body),
            None => {
                self.body.get_mut().waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

impl<'a> RequestBody {
    pub fn get_body(self: Arc<Self>) -> GetBody {
        GetBody { body: self }
    }

    pub fn try_get_body(self: Arc<Self>) -> Option<Arc<Option<Vec<u8>>>> {
        if !&self.is_written.load(Relaxed) {
            return None;
        }

        return Some(self.bytes.clone());
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
