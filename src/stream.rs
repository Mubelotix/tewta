use std::{sync::Arc, task::Poll, pin::Pin};
use async_mutex::Mutex;
use futures::{future::BoxFuture, FutureExt};
use tokio::io::{AsyncRead, AsyncWrite};

#[cfg(not(feature = "test"))]
pub type TcpStream = tokio::net::TcpStream;
#[cfg(feature = "test")]
pub type TcpStream = testing::TestStream;

/// While testing, we build a fake network of thousands of nodes.  
/// We replace the implementation of TcpStream with a local fake stream that's faster and more scalable.  
#[cfg(feature = "test")]
mod testing {
    use super::*;

    /// A fake [TcpStream] used for [testing].  
    /// Implements [`AsyncRead`] and [`AsyncWrite`].
    pub struct TestStream {
        inbound: Arc<Mutex<Vec<u8>>>,
        outbound: Arc<Mutex<Vec<u8>>>,
    
        inbound_lock_fut: Option<BoxFuture<'static, async_mutex::MutexGuardArc<Vec<u8>>>>,
        outbound_lock_fut: Option<BoxFuture<'static, async_mutex::MutexGuardArc<Vec<u8>>>>,
    }
    
    impl TestStream {
        pub fn new() -> (Self, Self) {
            let inbound = Arc::new(Mutex::new(Vec::new()));
            let outbound = Arc::new(Mutex::new(Vec::new()));
    
            (
                TestStream {
                    inbound: inbound.clone(),
                    outbound: outbound.clone(),
                    inbound_lock_fut: None,
                    outbound_lock_fut: None,
                },
                TestStream {
                    inbound: outbound,
                    outbound: inbound,
                    inbound_lock_fut: None,
                    outbound_lock_fut: None,
                },
            )
        }
    }
    
    impl AsyncRead for TestStream {
        /// WARNING: No notification will be sent when data becomes unavailable.  
        /// This behavior is NOT expected by the trait.
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            if self.inbound_lock_fut.is_none() {
                let self_inbound = Arc::clone(&self.inbound);
                self.inbound_lock_fut = Some(async move { self_inbound.lock_arc().await }.boxed());
            }
    
            if let Poll::Ready(mut inbound) = self.inbound_lock_fut.as_mut().unwrap().as_mut().poll(cx) {
                if buf.remaining() < inbound.len() {
                    return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Buffer too small")));
                }
                buf.put_slice(inbound.as_ref());
                inbound.clear();
    
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        }
    }
    
    impl AsyncWrite for TestStream {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize, std::io::Error>> {
            if self.outbound_lock_fut.is_none() {
                let self_outbound = Arc::clone(&self.outbound);
                self.outbound_lock_fut = Some(async move { self_outbound.lock_arc().await }.boxed());
            }
    
            if let Poll::Ready(mut outbound) = self.outbound_lock_fut.as_mut().unwrap().as_mut().poll(cx) {
                outbound.extend_from_slice(buf);
                Poll::Ready(Ok(buf.len()))
            } else {
                Poll::Pending
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
