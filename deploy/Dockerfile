# RemoteACC bootstrapd – multi-stage build
FROM rust:1-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock* ./
COPY crates/ crates/
RUN cargo build --release -p racc-bootstrapd

FROM debian:bookworm-slim
RUN useradd --system --no-create-home --shell /usr/sbin/nologin racc
COPY --from=builder /src/target/release/racc-bootstrapd /usr/local/bin/
VOLUME /data
EXPOSE 8443/udp
USER racc
ENTRYPOINT ["racc-bootstrapd"]
CMD ["--bind", "0.0.0.0:8443", "--dump-cert", "/data/bootstrap.der"]
