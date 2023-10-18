export LIB_DIR=$(PWD)/target/debug
export LIB_NAME=jequi_go
export HANDLER_PATH=../handle

clear:
	-rm $(LIB_DIR)/$(LIB_NAME).a
	-rm $(LIB_DIR)/$(LIB_NAME).h
	-rm $(LIB_DIR)/$(LIB_NAME).so

go_setup:
	cd ./go/jequi \
	&& go generate \
	&& go mod edit -replace github.com/handle=$(HANDLER_PATH) \
	&& go mod tidy

go: clear go_setup
	cd ./go/jequi && go build -o $(LIB_DIR)/$(LIB_NAME).so -ldflags='-shared'

run: go
	cargo run

reload:
	kill -SIGHUP $(shell cat jequi.pid)

example_static_files:
	cd example/static_files && cargo run -- -C ../../

