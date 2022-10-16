package handle

import "github.com/jequi_go"

func HandleResponse(r jequi_go.Response) {
	r.SetHeader("hello", "world")
}
