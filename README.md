# Jequi

Flexible web server written in rust that allows dynamic configuration extended by plugins

- /server -> execution starts here, it is responsible for running the web server, it calls `jequi`
- /jequi -> most functionality is here, it has all the objects and functions to allow jequi to function
- /plugins -> extra functionality to handle requests, called by `server`, calls `jequi` and `go` (for now everything is in one place but in the future there would be many plugins, executing go code can be an example of a plugin)
- /api -> the jequi api, code written by the user will call this api using `jequi_go`, this api will call functions defined in `jequi`
- /go -> has the code that will be called by jequi `server` and it can call functions from jequi `api` also
    - /go/jequi -> has the exported functions that jequi `server` will call and it calls the code written by the user (`handle`)
    - /go/jequi_go -> responsible for calling jequi `api`, the code written by the user (`handle`) will call the functions declared here
    - /go/handle -> code written by the user, it can call jequi `api` using `jequi_go`

`make run` and then `curl https://127.0.0.1:7878 -k` to test it

you can play with `/go/handle` to see the go code being executed by jequi