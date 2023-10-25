package main

import "C"
import (
	"unsafe"

	"github.com/jequi_go"
)

//export HandleRequest
func HandleRequest(req_pointer unsafe.Pointer, resp_pointer unsafe.Pointer) {
	req := jequi_go.NewRequest(req_pointer)
	resp := jequi_go.NewResponse(resp_pointer)
	resp.SetHeader("test", req.GetUri())
	resp.WriteBody("hello")
	resp.SetStatus(200)
}

func main() {}
