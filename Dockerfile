# ========================================================
# Multi-stage Dockerfile cho ứng dụng rust-mantis-bt
# Tối ưu kích thước runtime (< 40MB) và tốc độ build bằng cache
# ========================================================

# Stage 1: Build binary
FROM rust:1-bookworm AS builder
WORKDIR /app

# Cache dependency layer bằng cách build dummy project trước
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub fn dummy() {}" > src/lib.rs && \
    cargo build --release && \
    rm -rf src

# Copy toàn bộ mã nguồn thật và biên dịch
COPY src ./src
RUN touch src/main.rs src/lib.rs && \
    cargo build --release

# Stage 2: Runtime image siêu nhẹ
FROM debian:bookworm-slim
WORKDIR /app

# Cài đặt SSL root certificates và timezone
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates tzdata && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/rust-mantis-bt /usr/local/bin/rust-mantis-bt

# Cấu hình biến môi trường mặc định để lắng nghe trên mọi network interface
ENV MANTIS_ADDR=0.0.0.0:3000 \
    MANTIS_DB_PATH=/data/mantis.db \
    MANTIS_UPLOAD_DIR=/data/uploads

EXPOSE 3000
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/rust-mantis-bt"]
