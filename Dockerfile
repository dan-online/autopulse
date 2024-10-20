# Chef dependencies
FROM rust as planner
WORKDIR app

RUN cargo install cargo-chef 
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Install dependencies
FROM rust as cacher
WORKDIR app
RUN cargo install cargo-chef
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Build autopulse
FROM rust:1.82.0 as builder

WORKDIR /usr/src/
RUN USER=root cargo new --bin autopulse
WORKDIR /usr/src/autopulse

# Compile dependencies
COPY Cargo.toml Cargo.lock ./

# Copy source and build
COPY src src
COPY migrations migrations

# Build dependencies
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
RUN rm -rf target/release/autopulse*
RUN cargo build --locked --release

# Run application
FROM ubuntu:noble

WORKDIR /app

RUN apt-get update -y && apt-get install -y ca-certificates libssl-dev libpq-dev --no-install-recommends && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/autopulse/target/release/autopulse /usr/local/bin/

EXPOSE 2875

CMD ["/usr/local/bin/autopulse"]