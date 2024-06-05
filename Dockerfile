FROM rustlang/rust:nightly-slim as build

RUN apt update && apt install libssl-dev

# statically link against openssl
ENV OPENSSL_STATIC=1
ENV OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu/
ENV OPENSSL_INCLUDE_DIR=/usr/include/x86_64-linux-gnu/

COPY ./ .

RUN cargo build --release

FROM debian:stable-slim

COPY --from=build /etc/jequi /etc/jequi
COPY --from=build /target/release/server /etc/jequi/server
COPY --from=build /target/release/libjequi.so /etc/jequi/libjequi.so

WORKDIR /etc/jequi

ENTRYPOINT ["./server"]
