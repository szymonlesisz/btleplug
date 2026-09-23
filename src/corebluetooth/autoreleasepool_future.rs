use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// A future polled and destroyed inside fresh autorelease pools.
///
/// Values kept across polls or returned as output must own their Objective-C
/// references. Teardown of state borrowed by the future remains the caller's
/// responsibility.
pub(super) struct AutoreleasePoolFuture<F> {
    future: Option<Pin<Box<F>>>,
}

impl<F: Future> AutoreleasePoolFuture<F> {
    pub(super) fn new(future: F) -> Self {
        Self {
            future: Some(Box::pin(future)),
        }
    }
}

impl<F: Future> Future for AutoreleasePoolFuture<F> {
    type Output = F::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        objc2::rc::autoreleasepool(|_| {
            let poll = self
                .future
                .as_mut()
                .expect("polled AutoreleasePoolFuture after completion")
                .as_mut()
                .poll(cx);
            if poll.is_ready() {
                drop(self.future.take());
            }
            poll
        })
    }
}

impl<F> Drop for AutoreleasePoolFuture<F> {
    fn drop(&mut self) {
        if let Some(future) = self.future.take() {
            objc2::rc::autoreleasepool(|_| drop(future));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc2::{AnyThread, define_class, msg_send, rc::Retained};
    use objc2_foundation::{NSObject, NSObjectProtocol};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll, Waker};

    struct DropCounter(Arc<AtomicUsize>);

    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = AnyThread]
        #[ivars = DropCounter]
        struct AutoreleaseProbe;

        unsafe impl NSObjectProtocol for AutoreleaseProbe {}
    );

    fn retained_probe(drops: &Arc<AtomicUsize>) -> Retained<AutoreleaseProbe> {
        let allocated = AutoreleaseProbe::alloc().set_ivars(DropCounter(drops.clone()));
        unsafe { msg_send![super(allocated), init] }
    }

    fn autorelease_probe(drops: &Arc<AtomicUsize>) {
        let _ = Retained::autorelease_ptr(retained_probe(drops));
    }

    #[test]
    fn autoreleasepool_future_drains_after_pending_and_ready() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut polls = 0;
        let inner = std::future::poll_fn(|_| {
            autorelease_probe(&drops);
            polls += 1;
            if polls == 1 {
                Poll::Pending
            } else {
                Poll::Ready(42)
            }
        });
        let mut future = std::pin::pin!(AutoreleasePoolFuture::new(inner));
        let mut context = Context::from_waker(Waker::noop());

        assert_eq!(future.as_mut().poll(&mut context), Poll::Pending);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(future.as_mut().poll(&mut context), Poll::Ready(42));
        assert_eq!(drops.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn autoreleasepool_future_preserves_retained_state_and_output() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut retained: Option<Retained<AutoreleaseProbe>> = None;
        let inner = std::future::poll_fn(|_| {
            if let Some(object) = retained.take() {
                let _ = Retained::autorelease_ptr(object.clone());
                Poll::Ready(object)
            } else {
                let object = retained_probe(&drops);
                let _ = Retained::autorelease_ptr(object.clone());
                retained = Some(object);
                Poll::Pending
            }
        });
        let mut future = Box::pin(AutoreleasePoolFuture::new(inner));
        let mut context = Context::from_waker(Waker::noop());

        assert!(future.as_mut().poll(&mut context).is_pending());
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        let Poll::Ready(object) = future.as_mut().poll(&mut context) else {
            panic!("expected retained output");
        };
        drop(future);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(object);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn autoreleasepool_future_pools_cancellation() {
        struct AutoreleaseOnDrop(Arc<AtomicUsize>);

        impl Drop for AutoreleaseOnDrop {
            fn drop(&mut self) {
                autorelease_probe(&self.0);
            }
        }

        let drops = Arc::new(AtomicUsize::new(0));
        let cleanup = AutoreleaseOnDrop(drops.clone());
        let inner = std::future::poll_fn(move |_| {
            let _ = &cleanup;
            Poll::<()>::Pending
        });
        let mut future = Box::pin(AutoreleasePoolFuture::new(inner));
        let mut context = Context::from_waker(Waker::noop());

        assert!(future.as_mut().poll(&mut context).is_pending());
        drop(future);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}
