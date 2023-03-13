FROM rust:latest as builder
WORKDIR /app
COPY . .
RUN apt update -y && apt install musl-tools -y
RUN rustup update nightly && rustup override set nightly
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo install --path . --root . --target x86_64-unknown-linux-musl

FROM alpine:latest AS runtime
COPY --from=builder /app/bin/telegram_bot /usr/local/bin/telegram_bot
ENV TELOXIDE_TOKEN=
ENV API_HOST=
ENV RUST_LOG=debug
CMD ["myapp"]
