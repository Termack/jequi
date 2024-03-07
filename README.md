# Jequi

Jequi is a web server written in rust that focus on stability and flexibility. (It is not production ready yet but feel free to explore)

One of the main focus of Jequi is being very flexible and extensible, the way it does this is via plugins, plugins can add functionality to each request in a way that someone can write a plugin without needing to change the core of Jequi. For example the jequi_proxy adds the functionality for jequi to be used as a proxy and jequi_go plugin adds functionality of executing go code for each request and jequi_go can execute code in the proxy phase of jequi_proxy without needing to change jequi_proxy code.

## Features

- HTTP1 and HTTP2 support
- Proxy using jequi_proxy plugin
- Executing golang code using jequi_go plugin
  - You can define upstreams for jequi_proxy using jequi_go
- Serving static files using jequi_serve_static plugin

### Future plans

- Logging and metrics plugin
- Allow configuration with multiple files
- Javascript plugin
- Websocket support for proxy
- Load balancer plugin
- Plugin that generates certificate


## How to use jequi

You must have Rust installed, first compile the binary: `cargo build --release`

If you want to use the jequi_go plugin, compile the go shared library using: `make go`

Then run the compiled binary: `target/release/server`

It will use the file `conf.yaml` in your current directory, you can change the config file and then reload it while the server is still running with `make reload`

## Writing a plugin for jequi

todo

## Directory structure of this repository

- /server -> execution starts here, it is responsible for running the web server, it calls `jequi`
- /jequi -> most functionality is here, it has all the objects and functions to allow jequi to function
- /api -> the jequi api, language plugins like `jequi_go` will call functions defined here, this api will call functions defined in `jequi`
- /plugins -> it has some proc macros for using plugins
  - /jequi_go -> the jequi_go plugin
    - /go -> the go code that will be executed is here
      - /handle -> **this is the code that has the functions that should be written by the user, feel free to play with it**
      - /jequi -> responsible for defining the interface that `/plugins/jequi_go` will call
      - /jequi_go -> defines the go api that will call the jequi api (rust)

## Configuration

Jequi uses yaml for its configuration, here's an example:

```
tls_active: true
proxy_address: "www.example1.com"
host:
  jequi.com:
    http2: true
    uri:
      /api:
        go_library_path: "target/debug/jequi_go.so"
        proxy_address: "www.example2.com"
    static_files_path: "test/"
uri:
  /api/v2:
    go_library_path: "target/debug/jequi_go.so"
```

As you can see, jequi configuration allow some scopes, there's the default, host and uri. The configuration used is the most specific, so for example, a request to `jequi.com/api/bla` will execute go code from `target/debug/jequi_go.so` and then proxy the request to `www.example2.com` and a request to `jequi.com/hello` will serve a file from `test/`.

### Configuration Options

tls_active
----------
**scope:** default

**type:** bool

Defines if tls is active for server.

ip
----------
**scope:** default

**type:** string

Defines the ip address that server will listen.


port
----------
**scope:** default

**type:** string

Defines the port that server will listen.

http2
----------
**scope:** default, host

**type:** bool

Defines if the server accepts http2.

chunk_size
----------
**scope:** default, host, uri

**type:** int

Defines the maximum chunk size for http responses.

static_files_path
----------
**From jequi_serve_static plugin**

**scope:** default, host, uri

**type:** string

Sets the path to serve static files, if the path is a directory it will serve the files based on the request uri, if it is a file, it will serve the file always.


proxy_address
----------
**From jequi_proxy plugin**

**scope:** default, host, uri

**type:** string

Define the upstream address that the server will proxy, the address can be an ip or domain and can have a port specified.


go_library_path
----------
**From jequi_go plugin**

**scope:** default, host, uri

**type:** string

Define the path of the compiled go shared lib that will be used to execute the go functions.

## Api

Jequi has an api that allows for language plugins (like jequi_go for example) to communicate with it via FFI similiar to what openresty does with lua.

### Api Documentation

set_response_header
--------------------

```
set_response_header(
    response,
    header: string,
    value: string,
)
```

Set a response header for the response.


set_response_status
--------------------

```
set_response_status(
    response,
    status: int,
)
```

Set the response status.


write_response_body
--------------------

```
write_response_body(
    response,
    content: string,
)
```

Write content into the response body buffer, if this function is called multiple times it will append content to the buffer.


get_request_header
--------------------

```
get_request_header(
    request,
    header: string,
) -> string
```

Returns the value of a request header, if the header doesn't exist it will return an empty string.


get_request_body
--------------------

```
get_request_body(
    request,
) -> string
```

Returns the request body, if it wasn't read yet it will wait until it is read and return it as a string.


get_request_uri
--------------------

```
get_request_uri(
    request,
) -> string
```

Returns the request uri as a string.


get_request_method
--------------------

```
get_request_method(
    request,
) -> string
```

Returns the request method as a string.


set_request_uri
--------------------
**From jequi_proxy plugin**

```
set_request_uri(
    request,
    uri: string
)
```

Overwrites the request uri, so when the request is made to the proxy upstream it uses this new uri.

