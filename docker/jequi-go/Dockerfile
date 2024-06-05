FROM rustlang/rust:nightly-slim as build

RUN apt update && apt install libssl-dev

# statically link against openssl
ENV OPENSSL_STATIC=1
ENV OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu/
ENV OPENSSL_INCLUDE_DIR=/usr/include/x86_64-linux-gnu/

COPY . .

RUN cargo build --release

FROM debian:stable-slim

COPY --from=build /etc/jequi /etc/jequi
COPY --from=build /Makefile /etc/jequi/Makefile
COPY --from=build /plugins/jequi_go/go /etc/jequi/plugins/jequi_go/go
COPY --from=build /target/release/server /etc/jequi/server
COPY --from=build /target/release/libjequi.so /etc/jequi/libjequi.so
COPY docker/jequi-go/start.sh /etc/jequi/start.sh

COPY --from=golang:1.22-bookworm /usr/local/go/ /usr/local/go/

ENV CGO_ENABLED=1

RUN apt update && apt install make && apt install -y gcc

ENV PATH="/usr/local/go/bin:${PATH}"

WORKDIR /etc/jequi

CMD ["./start.sh"]
