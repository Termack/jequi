package handle

import (
	"fmt"

	"github.com/jequi_go"
)

func HandleRequest(req jequi_go.Request, resp jequi_go.Response) {
	resp.SetHeader("hello", "world")
	resp.WriteBody("Hello Waaaaaaorld!")
	resp.SetStatus(404)
	fmt.Printf("Method: %q, Uri: %q, User-Agent: %q, Body: %q",
		req.GetMethod(),
		req.GetUri(),
		req.GetHeader("User-Agent"),
		req.GetBody())
}
