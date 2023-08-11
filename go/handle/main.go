package handle

import (
	"fmt"

	"github.com/jequi_go"
)

func HandleResponse(r jequi_go.Response) {
	r.SetHeader("hello", "world")
	fmt.Println("this came from go")
}
