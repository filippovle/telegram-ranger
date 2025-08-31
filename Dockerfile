# ---------- builder ----------
FROM rust:1.80-bookworm AS builder
WORKDIR /app

# Опционально: OpenSSL dev не нужен, если всё на rustls.
# Оставлю на всякий случай (не ломает сборку).
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Сначала метаданные — лучше кэшируется
COPY Cargo.toml Cargo.lock ./
# Если есть workspace — возможно, придётся копировать и другие Cargo.toml
COPY src ./src

# Релизная сборка (использует ваш [profile.release] из Cargo.toml)
RUN cargo build --release

# ---------- runtime ----------
FROM debian:bookworm-slim
WORKDIR /app

# Нужны только сертификаты и зона времени
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates tzdata && rm -rf /var/lib/apt/lists/*

# Бинарник
COPY --from=builder /app/target/release/telegram-ranger /usr/local/bin/telegram-ranger

# Каталог для состояния (будет смонтирован томом)
ENV STATE_FILE=/data/state.json \
    RUST_LOG=info
VOLUME ["/data"]

# Портов не нужно — бот делает long-polling
ENTRYPOINT ["telegram-ranger"]
