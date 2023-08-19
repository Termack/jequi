package jequi_go

//#cgo LDFLAGS: -L${SRCDIR}/../../target/debug -Wl,-rpath=${SRCDIR}/../../target/debug -ljequi -ldl
//#include <stdlib.h>
//extern void set_response_header(void* resp, char* header, char* value);
//extern void write_response_body(void* resp, char* value);
//extern void set_response_status(void* resp, int status);
import "C"
import (
	"unsafe"
)

type Response struct {
	pointer unsafe.Pointer
}

func NewResponse(pointer unsafe.Pointer) Response {
	return Response{
		pointer: pointer,
	}
}

func (r *Response) SetHeader(header, value string) {
	C.set_response_header(r.pointer, C.CString(header), C.CString(value))
}

func (r *Response) WriteBody(value string) {
	C.write_response_body(r.pointer, C.CString(value))
}

func (r *Response) SetStatus(status int) {
	C.set_response_status(r.pointer, C.int(status))
}
