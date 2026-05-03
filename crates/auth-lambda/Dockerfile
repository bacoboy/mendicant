FROM ghcr.io/cargo-lambda/cargo-lambda:latest AS builder
RUN apt-get update && apt-get install -y pkg-config perl && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# ── Dependency cache layer ─────────────────────────────────────────────────────
# Copy only manifests so this layer is only invalidated when Cargo.toml or
# Cargo.lock changes — not on every source edit. Stub sources let cargo
# resolve and compile all external dependencies without the real application
# code. The compiled artifacts sit in /app/target for the next stage.
COPY Cargo.toml Cargo.lock ./
COPY crates/domain/Cargo.toml       crates/domain/
COPY crates/db/Cargo.toml           crates/db/
COPY crates/auth-lambda/Cargo.toml  crates/auth-lambda/
COPY crates/users-lambda/Cargo.toml crates/users-lambda/
COPY crates/bootstrap/Cargo.toml    crates/bootstrap/

RUN mkdir -p \
        crates/domain/src \
        crates/db/src \
        crates/auth-lambda/src \
        crates/users-lambda/src \
        crates/bootstrap/src \
    && touch crates/domain/src/lib.rs \
             crates/db/src/lib.rs \
    && printf 'fn main() {}\n' | tee \
        crates/auth-lambda/src/main.rs \
        crates/users-lambda/src/main.rs \
        crates/bootstrap/src/main.rs > /dev/null

RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    cargo lambda build --release -p auth-lambda --arm64

# ── Application build ─────────────────────────────────────────────────────────
# Re-runs when any source file changes, but external deps are already compiled
# above so only workspace crates (domain, db, auth-lambda) are recompiled.
# touch ensures cargo sees real sources as newer than the empty stubs above.
COPY crates/ crates/
RUN find crates -name "*.rs" -exec touch {} +
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    cargo lambda build --release -p auth-lambda --arm64 \
    && cp /app/target/lambda/auth-lambda/bootstrap /bootstrap

FROM public.ecr.aws/lambda/provided:al2023
COPY --from=builder /bootstrap /var/runtime/bootstrap
CMD ["bootstrap"]
