package main

//go:generate go run generate.go github.com/handle

import "C"
import (
	"unsafe"

	"github.com/jequi_go"
)

//export HandleResponse
func HandleResponse(resp unsafe.Pointer) {
	r := jequi_go.NewResponse(resp)
	handleResponse(r)
}

func main() {}
