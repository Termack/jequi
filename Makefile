export LIB_DIR=$(PWD)/target/debug
export LIB_NAME=jequi_go

clear:
	-rm $(LIB_DIR)/lib$(LIB_NAME).a
	-rm $(LIB_DIR)/lib$(LIB_NAME).h
	-rm $(LIB_DIR)/lib$(LIB_NAME).so

go_setup:
	cd ./go/jequi \
	&& go generate \
	&& go mod edit -replace github.com/handle=../handle \
	&& go mod tidy

static: clear go_setup
	cd ./go/jequi && go build -buildmode=c-archive -o $(LIB_DIR)/lib$(LIB_NAME).a

dylib: clear go_setup
	cd ./go/jequi && go build -o $(LIB_DIR)/lib$(LIB_NAME).so -buildmode=c-shared

run: dylib
	cargo run