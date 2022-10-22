use jequi::Response;
use std::ffi::CStr;
use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn set_header(
    resp: *mut Response,
    header: *const c_char,
    value: *const c_char,
) -> i32 {
    let resp = unsafe {
        assert!(!resp.is_null());
        &mut *resp
    };
    resp.set_header(
        unsafe { CStr::from_ptr(header) }.to_str().unwrap(),
        unsafe { CStr::from_ptr(value) }.to_str().unwrap(),
    );
    0
}
