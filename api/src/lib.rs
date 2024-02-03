use jequi::{Request, Response};
use jequi_proxy::*; // TODO: proc macro to use all plugins to load custom apis
use std::ffi::{c_int, CStr, CString};
use std::os::raw::c_char;

unsafe fn get_object_from_pointer<'a, T>(obj: *mut T) -> &'a mut T {
    assert!(!obj.is_null());
    unsafe { &mut *obj }
}

#[no_mangle]
pub unsafe extern "C" fn set_response_header(
    resp: *mut Response,
    header: *const c_char,
    value: *const c_char,
) {
    let resp = unsafe { get_object_from_pointer(resp) };
    resp.set_header(
        unsafe { CStr::from_ptr(header) }.to_str().unwrap(),
        unsafe { CStr::from_ptr(value) }.to_str().unwrap(),
    );
}

#[no_mangle]
pub unsafe extern "C" fn set_response_status(resp: *mut Response, int: c_int) {
    let resp = unsafe { get_object_from_pointer(resp) };
    resp.status = int as usize;
}

#[no_mangle]
pub unsafe extern "C" fn write_response_body(resp: *mut Response, string: *const c_char) {
    let resp = unsafe { get_object_from_pointer(resp) };
    resp.write_body(unsafe { CStr::from_ptr(string) }.to_bytes())
        .unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn get_request_header(
    req: *mut Request,
    header: *const c_char,
) -> *const c_char {
    let req = unsafe { get_object_from_pointer(req) };
    let value = match req.get_header(unsafe { CStr::from_ptr(header) }.to_str().unwrap()) {
        Some(value) => value.to_str().unwrap(),
        None => "",
    };
    CString::new(value).unwrap().into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn get_request_body(req: *mut Request) -> *const c_char {
    let req = unsafe { get_object_from_pointer(req) };
    CString::new(req.get_body().unwrap_or(&"".to_string()).as_str())
        .unwrap()
        .into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn get_request_uri(req: *mut Request) -> *const c_char {
    let req: &mut Request = unsafe { get_object_from_pointer(req) };
    CString::new(req.uri.as_str()).unwrap().into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn get_request_method(req: *mut Request) -> *const c_char {
    let req: &mut Request = unsafe { get_object_from_pointer(req) };
    CString::new(req.method.as_str()).unwrap().into_raw()
}
