package jequi_go

//#cgo LDFLAGS: -L${SRCDIR}/../../target/debug -Wl,-rpath=${SRCDIR}/../../target/debug -ljequi -ldl
//#include <stdint.h>
//extern int32_t set_header(void* resp,char* header, char* value);
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
	C.set_header(r.pointer, C.CString(header), C.CString(value))
}
