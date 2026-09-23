#![allow(non_camel_case_types)]
use std::os::raw::{c_char, c_ulong, c_void};

pub type dispatch_object_s = c_void;
pub type dispatch_queue_t = *mut dispatch_object_s;
pub type dispatch_queue_attr_t = *const dispatch_object_s;

pub const DISPATCH_QUEUE_SERIAL: dispatch_queue_attr_t = 0 as dispatch_queue_attr_t;
pub const DISPATCH_AUTORELEASE_FREQUENCY_WORK_ITEM: c_ulong = 1;

unsafe extern "C" {
    pub fn dispatch_queue_attr_make_with_autorelease_frequency(
        attr: dispatch_queue_attr_t,
        frequency: c_ulong,
    ) -> dispatch_queue_attr_t;
    pub fn dispatch_queue_create(
        label: *const c_char,
        attr: dispatch_queue_attr_t,
    ) -> dispatch_queue_t;
    pub fn dispatch_release(object: *mut dispatch_object_s);

    #[cfg(test)]
    pub fn dispatch_async_f(
        queue: dispatch_queue_t,
        context: *mut c_void,
        work: unsafe extern "C" fn(*mut c_void),
    );
}

// TODO: Do we need to link to AppKit here?
#[cfg_attr(target_os = "macos", link(name = "AppKit", kind = "framework"))]
unsafe extern "C" {}
