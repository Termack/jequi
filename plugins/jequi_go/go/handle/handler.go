package handle

import (
	"fmt"

	"github.com/jequi_go"
)

func HandleRequest(req jequi_go.Request, resp jequi_go.Response) {
	// fmt.Println(req.GetUri(), strings.HasSuffix(req.GetUri(), ".js"))

	// resp.SetHeader("hello", "world")
	// // resp.WriteBody("Hello World!\n")
	// // resp.SetStatus(404)
	fmt.Printf("Method: %q, Uri: %q, User-Agent: %q, Body: %q\n",
		req.GetMethod(),
		req.GetUri(),
		req.GetHeader("User-Agent"),
		req.GetBody())
}

func HandleProxyRequest(req jequi_go.Request, resp jequi_go.Response) *string {
	// val := strings.SplitN(req.GetUri(), "/", 3)
	// if len(val) == 1 || (len(val) == 2 && val[1] == "") {
	// 	return nil
	// }
	//
	// address := val[1]
	// newUri := "/"
	//
	// if len(val) == 3 {
	// 	newUri = newUri + val[2]
	// }
	// req.SetUri(newUri)
	//
	// return &address
	return nil
}
