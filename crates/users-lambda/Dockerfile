FROM ghcr.io/cargo-lambda/cargo-lambda:latest AS builder
RUN apt-get update && apt-get install -y pkg-config perl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/app/target \
    cargo lambda build --release -p users-lambda --arm64 \
    && cp /app/target/lambda/users-lambda/bootstrap /bootstrap

FROM public.ecr.aws/lambda/provided:al2023
COPY --from=builder /bootstrap /var/runtime/bootstrap
CMD ["bootstrap"]
