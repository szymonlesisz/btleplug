use super::ffi;
use std::ptr::NonNull;

/// Owns the +1 reference returned by dispatch_queue_create and releases it on drop.
pub(super) struct CallbackQueue(NonNull<ffi::dispatch_object_s>);

impl CallbackQueue {
    /// Creates a serial callback queue with a +1 reference managed by CallbackQueue.
    /// Passing its pointer to another API does not transfer that reference; callers
    /// must keep the owner alive while the queue is needed and must not manually
    /// release its reference. Dropping the owner balances the reference automatically.
    pub(super) fn new() -> Self {
        let queue = unsafe {
            let attr = ffi::dispatch_queue_attr_make_with_autorelease_frequency(
                ffi::DISPATCH_QUEUE_SERIAL,
                ffi::DISPATCH_AUTORELEASE_FREQUENCY_WORK_ITEM,
            );
            ffi::dispatch_queue_create(c"CBqueue".as_ptr(), attr)
        };
        Self(NonNull::new(queue).expect("failed to create CoreBluetooth callback queue"))
    }

    /// Borrows the queue pointer without transferring ownership or retaining it.
    /// The pointer must not outlive this owner unless another reference keeps it alive.
    pub(super) fn as_ptr(&self) -> ffi::dispatch_queue_t {
        self.0.as_ptr()
    }
}

impl Drop for CallbackQueue {
    fn drop(&mut self) {
        unsafe { ffi::dispatch_release(self.as_ptr()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc2::{AnyThread, define_class, msg_send, rc::Retained};
    use objc2_foundation::{NSObject, NSObjectProtocol};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

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
        struct CallbackQueueAutoreleaseProbe;

        unsafe impl NSObjectProtocol for CallbackQueueAutoreleaseProbe {}
    );

    fn autorelease_probe(drops: &Arc<AtomicUsize>) {
        let allocated =
            CallbackQueueAutoreleaseProbe::alloc().set_ivars(DropCounter(drops.clone()));
        let object: Retained<CallbackQueueAutoreleaseProbe> =
            unsafe { msg_send![super(allocated), init] };
        let _ = Retained::autorelease_ptr(object);
    }

    #[test]
    fn callback_queue_drains_between_work_items() {
        use std::ffi::c_void;
        use std::sync::mpsc;
        use std::time::Duration;

        struct WorkItem {
            drops: Arc<AtomicUsize>,
            result: Option<mpsc::Sender<usize>>,
        }

        unsafe extern "C" fn run_work_item(context: *mut c_void) {
            let work = unsafe { Box::from_raw(context.cast::<WorkItem>()) };
            if let Some(result) = work.result {
                let _ = result.send(work.drops.load(Ordering::SeqCst));
            } else {
                autorelease_probe(&work.drops);
            }
        }

        let drops = Arc::new(AtomicUsize::new(0));
        let (sender, receiver) = mpsc::channel();
        let queue = CallbackQueue::new();
        for result in [None, Some(sender)] {
            let work = Box::new(WorkItem {
                drops: drops.clone(),
                result,
            });
            unsafe {
                ffi::dispatch_async_f(queue.as_ptr(), Box::into_raw(work).cast(), run_work_item);
            }
        }
        drop(queue);

        let dropped = receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("callback queue did not finish its work items");
        assert_eq!(dropped, 1);
    }
}
