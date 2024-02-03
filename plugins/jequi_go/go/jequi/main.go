package main

//go:generate go run generate.go github.com/handle

import "C"

import (
	"unsafe"

	"github.com/jequi_go"
)

//export HandleRequest
func HandleRequest(req_pointer unsafe.Pointer, resp_pointer unsafe.Pointer) {
	req := jequi_go.NewRequest(req_pointer)
	resp := jequi_go.NewResponse(resp_pointer)
	handleRequest(req, resp)
}

//export HandleProxyRequest
func HandleProxyRequest(req_pointer unsafe.Pointer, resp_pointer unsafe.Pointer) *C.char {
	req := jequi_go.NewRequest(req_pointer)
	resp := jequi_go.NewResponse(resp_pointer)
	address := handleProxyRequest(req, resp)
	if address != nil {
		return C.CString(*address)
	}
	return nil
}

func main() {}
