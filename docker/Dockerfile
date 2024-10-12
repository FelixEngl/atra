FROM rust:1-slim-bookworm as builder

RUN apt update; apt upgrade; apt -y install pkg-config libssl-dev clang llvm; apt autoremove
RUN mkdir atra

WORKDIR /atra

COPY ../atra ./atra
COPY ../external ./external
COPY ../iso_stopwords ./iso_stopwords
COPY ../svm ./svm
COPY ../warc ./warc
COPY ../text_processing ./text_processing
COPY ../Cargo.toml ./Cargo.toml
COPY ../logo.txt ./logo.txt

RUN cargo generate-lockfile
RUN cargo fetch
#RUN cargo build -p reqwest
#RUN cargo build -p rocksdb
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt update; apt upgrade; apt -y --no-install-recommends install libc6 openssl ca-certificates; apt autoremove

WORKDIR /atra

COPY --from=builder /atra/target/release .

#CMD ["./atra"]