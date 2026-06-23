# 1. Planner Stage: Prepare the recipe
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# 2. Cacher Stage: Build third-party dependencies
FROM chef AS cacher
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# 3. Builder Stage: Build our actual workspace binaries
FROM chef AS builder
COPY . .
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo

# Use build arguments to specify which workspace member and database URL
ARG SERVICE_NAME
ARG DATABASE_URL
RUN DATABASE_URL=$DATABASE_URL cargo build --release --package ${SERVICE_NAME}

# 4. Runtime Stage: A ultra-slim image to run the binary
FROM debian:trixie-slim AS runtime
WORKDIR /app

ARG SERVICE_NAME
# Copy binaries dynamically based on the service name
COPY --from=builder /app/target/release/api /app/api
COPY --from=builder /app/target/release/init /app/init
COPY ./raw ./raw

# Default fallback command
CMD ["./api"]
