# Stage 1: Build the Rust binary
FROM rust:1.95-bullseye as builder
WORKDIR /app
COPY . .
ENV SQLX_OFFLINE=true
# Build the application in release mode for maximum speed
RUN cargo build --release --bin init --bin api

# Stage 2: The minimal runtime environment
FROM debian:bookworm-slim
WORKDIR /app
# Install OpenSSL and CA certificates (required by sqlx for secure connections)
RUN apt-get update && apt-get install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy the binary from the builder stage
COPY --from=builder /app/target/release/init /app/init
COPY --from=builder /app/target/release/api /app/api
COPY ./raw ./raw
COPY ./migrations ./migrations
COPY ./.env ./.env

CMD ["./api"]
