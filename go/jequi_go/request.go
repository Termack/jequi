package jequi_go

//#cgo LDFLAGS: -L${SRCDIR}/../../target/debug -Wl,-rpath=${SRCDIR}/../../target/debug -ljequi -ldl
//#include <stdlib.h>
//extern char* get_request_header(void* req, char* header);
//extern char* get_request_body(void* req);
//extern char* get_request_uri(void* req);
//extern char* get_request_method(void* req);
import "C"
import (
	"unsafe"
)

type Request struct {
	pointer unsafe.Pointer
}

func NewRequest(pointer unsafe.Pointer) Request {
	return Request{
		pointer: pointer,
	}
}

func cstring_to_string(value_pointer *C.char) string {
	value := C.GoString(value_pointer)
	C.free(unsafe.Pointer(value_pointer))
	return value
}

func (r *Request) GetHeader(header string) string {
	value := C.get_request_header(r.pointer, C.CString(header))
	return cstring_to_string(value)
}

func (r *Request) GetBody() string {
	body := C.get_request_body(r.pointer)
	return cstring_to_string(body)
}

func (r *Request) GetUri() string {
	uri := C.get_request_uri(r.pointer)
	return cstring_to_string(uri)
}

func (r *Request) GetMethod() string {
	method := C.get_request_method(r.pointer)
	return cstring_to_string(method)
}
