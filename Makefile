export LIB_DIR=$(PWD)/target/debug
export LIB_NAME=jequi_go
export HANDLER_PATH=../handle

clear:
	-rm $(LIB_DIR)/$(LIB_NAME).a
	-rm $(LIB_DIR)/$(LIB_NAME).h
	-rm $(LIB_DIR)/$(LIB_NAME).so

go_setup:
	if ! [ -f /etc/jequi/libjequi.so ]; then \
		sudo cp target/debug/libjequi.so /etc/jequi/libjequi.so; \
	fi \
	&& cd ./plugins/jequi_go/go/jequi \
	&& go generate \
	&& go mod edit -replace github.com/handle=$(HANDLER_PATH) \
	&& go mod tidy

go: clear go_setup
	cd ./plugins/jequi_go/go/jequi && LIB_DIR=$(LIB_DIR) go build -o $(LIB_DIR)/$(LIB_NAME).so -buildmode=c-shared

run: go
	cargo run

reload:
	kill -SIGHUP $(shell cat jequi.pid)

example_static_files:
	cd example/static_files && cargo run -- -C ../../

