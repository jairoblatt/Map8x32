FROM rust:1.86.0-alpine AS deps
RUN apk add --no-cache musl-dev
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --target x86_64-unknown-linux-musl
RUN rm -rf src

FROM deps AS builder
COPY src ./src/
RUN touch src/main.rs
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM alpine:latest AS optimizer
RUN apk add --no-cache binutils
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/map8x32-server /map8x32-server
RUN strip --strip-all /map8x32-server
RUN chmod +x /map8x32-server

FROM scratch

COPY --from=optimizer /map8x32-server /map8x32-server

COPY --from=alpine:latest /etc/passwd /etc/passwd

EXPOSE 8080

USER 65534:65534

ENTRYPOINT ["/map8x32-server"]