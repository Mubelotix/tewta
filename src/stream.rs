use std::{sync::Arc, task::{Poll, Waker}, pin::Pin};
use async_mutex::{Mutex, MutexGuardArc};
use futures::{future::BoxFuture, FutureExt};
use tokio::io::{AsyncRead, AsyncWrite};
use log::*;

#[cfg(not(feature = "test"))]
pub type TcpStream = tokio::net::TcpStream;
#[cfg(feature = "test")]
pub type TcpStream = testing::TestStream;

/// While testing, we build a fake network of thousands of nodes.  
/// We replace the implementation of TcpStream with a local fake stream that's faster and more scalable.  
#[cfg(feature = "test")]
pub mod testing {
    use super::*;

    /// A fake [TcpStream] used for [testing].  
    /// Implements [`AsyncRead`] and [`AsyncWrite`].
    pub struct TestStream {
        inbound: Arc<Mutex<Vec<u8>>>,
        outbound: Arc<Mutex<Vec<u8>>>,
        to_wake_on_write: Arc<Mutex<Option<Waker>>>,
        waken_on_readable: Arc<Mutex<Option<Waker>>>,
    }

    impl TestStream {
        pub fn new() -> (Self, Self) {
            let inbound = Arc::new(Mutex::new(Vec::new()));
            let outbound = Arc::new(Mutex::new(Vec::new()));
            let to_wake_on_write = Arc::new(Mutex::new(None));
            let waken_on_readable = Arc::new(Mutex::new(None));
    
            (
                TestStream {
                    inbound: inbound.clone(),
                    outbound: outbound.clone(),
                    to_wake_on_write: to_wake_on_write.clone(),
                    waken_on_readable: waken_on_readable.clone(),
                },
                TestStream {
                    inbound: outbound,
                    outbound: inbound,
                    to_wake_on_write: waken_on_readable,
                    waken_on_readable: to_wake_on_write,
                },
            )
        }

        pub fn split(&mut self) -> (TestReadHalf, TestWriteHalf) {
            (
                TestReadHalf {
                    data: Arc::clone(&self.inbound),
                    waken_on_readable: Arc::clone(&self.waken_on_readable),
                    lock_fut: None,
                    waker_lock_fut: None,
                },
                TestWriteHalf {
                    data: Arc::clone(&self.outbound),
                    to_wake_on_write: Arc::clone(&self.to_wake_on_write),
                    wrote: false,
                    woke: false,
                    lock_fut: None,
                    waker_lock_fut: None,
                }
            )
        }
    }

    pub struct TestReadHalf {
        data: Arc<Mutex<Vec<u8>>>,
        waken_on_readable: Arc<Mutex<Option<Waker>>>,
        lock_fut: Option<BoxFuture<'static, MutexGuardArc<Vec<u8>>>>,
        waker_lock_fut: Option<BoxFuture<'static, MutexGuardArc<Option<Waker>>>>,
    }

    pub struct TestWriteHalf {
        data: Arc<Mutex<Vec<u8>>>,
        to_wake_on_write: Arc<Mutex<Option<Waker>>>,
        wrote: bool,
        woke: bool,
        lock_fut: Option<BoxFuture<'static, MutexGuardArc<Vec<u8>>>>,
        waker_lock_fut: Option<BoxFuture<'static, MutexGuardArc<Option<Waker>>>>,
    }

    impl AsyncRead for TestReadHalf {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            trace!("ReadHalf: polled");

            // TODO: optimization: don't update if unchanged
            // Updating waker
            if self.waker_lock_fut.is_none() {
                let self_waken_on_readable = Arc::clone(&self.waken_on_readable);
                self.waker_lock_fut = Some(async move { self_waken_on_readable.lock_arc().await }.boxed());
            }
            if let Poll::Ready(mut waker) = self.waker_lock_fut.as_mut().unwrap().as_mut().poll(cx) {
                self.waker_lock_fut = None;
                
                *waker = Some(cx.waker().clone());
                trace!("ReadHalf: waker updated");
            }
            
            // Checking readable
            if self.lock_fut.is_none() {
                let self_outbound = Arc::clone(&self.data);
                self.lock_fut = Some(async move { self_outbound.lock_arc().await }.boxed());
            }
            if let Poll::Ready(mut data) = self.lock_fut.as_mut().unwrap().as_mut().poll(cx) {
                self.lock_fut = None;
                
                if !data.is_empty() {
                    trace!("ReadHalf: data read");
                    if buf.remaining() < data.len() {
                        let size = buf.remaining();
                        buf.put_slice(&data[..size]);
                        data.drain(..size);
                    } else {
                        buf.put_slice(data.as_slice());
                        data.clear();
                    }
                    return Poll::Ready(Ok(()));
                } else {
                    trace!("ReadHalf: not readable")
                }
            }
            
            Poll::Pending
        }
    }

    impl AsyncWrite for TestWriteHalf {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize, std::io::Error>> {
            if !self.wrote {
                if self.lock_fut.is_none() {
                    let self_outbound = Arc::clone(&self.data);
                    self.lock_fut = Some(async move { self_outbound.lock_arc().await }.boxed());
                }

                if let Poll::Ready(mut data) = self.lock_fut.as_mut().unwrap().as_mut().poll(cx) {
                    self.lock_fut = None;
    
                    data.extend_from_slice(buf);
                    self.wrote = true;
                    trace!("WriteHalf: wrote {} bytes", buf.len());
                }
            }
            
            if self.wrote && !self.woke {
                if self.waker_lock_fut.is_none() {
                    let self_to_wake_on_write = Arc::clone(&self.to_wake_on_write);
                    self.waker_lock_fut = Some(async move { self_to_wake_on_write.lock_arc().await }.boxed());
                }

                if let Poll::Ready(waker) = self.waker_lock_fut.as_mut().unwrap().as_mut().poll(cx) {
                    self.waker_lock_fut = None;
                    
                    if let Some(waker) = waker.clone() {
                        waker.wake();
                        trace!("WriteHalf: woke read half");
                    } else {
                        warn!("WriteHalf: did not wake");
                    }
                    self.woke = true;
                }
            }
            
            match self.wrote && self.woke {
                true => {
                    self.wrote = false;
                    self.woke = false;
                    Poll::Ready(Ok(buf.len()))
                },
                false => Poll::Pending,
            }
        }
    
        fn poll_flush(self: Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> Poll<Result<(), std::io::Error>> {
            Poll::Ready(Ok(()))
        }
    
        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> Poll<Result<(), std::io::Error>> {
            unimplemented!("Shutdown on virtual testing streams is not implemented");
        }
    }
}
