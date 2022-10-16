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
	cargo build --lib
	cd ./go/jequi && go build -buildmode=c-archive -o $(LIB_DIR)/lib$(LIB_NAME).a
	LIB_TYPE=static cargo build --bin jequi

dynamic: clear go_setup
	cargo build --lib
	cd ./go/jequi && go build -o $(LIB_DIR)/lib$(LIB_NAME).so -buildmode=c-shared
	LIB_TYPE=dynamic cargo run --bin jequi
