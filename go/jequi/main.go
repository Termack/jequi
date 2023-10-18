package main

//go:generate go run generate.go github.com/handle

import "C"

import (
	"fmt"
	"runtime"
	"runtime/debug"
	"unsafe"
	// "github.com/jequi_go"
)

//export HandleRequest
func HandleRequest(req_pointer unsafe.Pointer, resp_pointer unsafe.Pointer) {
	fmt.Println("bleeeaaaaaaa")
	//  req := jequi_go.NewRequest(req_pointer)
	// resp := jequi_go.NewResponse(resp_pointer)
	// handleRequest(req, resp)
}

//export Close
func Close() {
	fmt.Println(runtime.NumGoroutine())
	debug.SetGCPercent(-1)
	// runtime.LockOSThread()
	fmt.Println(runtime.NumGoroutine())
}

func main() {}
