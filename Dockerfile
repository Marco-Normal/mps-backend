# Stage 1: Build the Rust binary
FROM rust:1.95-bullseye as builder
WORKDIR /app
COPY . .
# Build the application in release mode for maximum speed
RUN cargo build --release

# Stage 2: The minimal runtime environment
FROM debian:bookworm-slim
WORKDIR /app
# Install OpenSSL and CA certificates (required by sqlx for secure connections)
RUN apt-get update && apt-get install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy the binary from the builder stage
COPY --from=builder /app/target/release/mps-backend /app/csv_to_sql
COPY ./raw ./raw
COPY ./migrations ./migrations
COPY ./.env ./.env

CMD ["./csv_to_sql"]