FROM --platform=$BUILDPLATFORM rust:1.88 AS chef
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM --platform=$BUILDPLATFORM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .

# Install `dx`
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
RUN cargo binstall dioxus-cli --root /.cargo -y --force
ENV PATH="/.cargo/bin:$PATH"

# Create the final bundle folder. Bundle always executes in release mode with optimizations enabled
RUN dx bundle --platform web --package web

# Use nginx to serve static files
FROM nginx:1.27-bookworm AS runtime

# Remove default nginx config and html
RUN rm -rf /etc/nginx/conf.d/default.conf /usr/share/nginx/html/*

# Copy our app files
COPY --from=builder /app/target/dx/web/release/web/public/ /usr/share/nginx/html/

# Create nginx config for SPA with WASM support
RUN echo 'server { \
    listen 0.0.0.0:8080; \
    server_name _; \
    root /usr/share/nginx/html; \
    index index.html; \
    \
    # Add WASM MIME type \
    location ~* \.wasm$ { \
        add_header Content-Type application/wasm; \
        expires 1y; \
        add_header Cache-Control "public, immutable"; \
    } \
    \
    # Add JS MIME type \
    location ~* \.js$ { \
        add_header Content-Type application/javascript; \
        expires 1y; \
        add_header Cache-Control "public, immutable"; \
    } \
    \
    # SPA fallback \
    location / { \
        try_files $uri $uri/ /index.html; \
    } \
}' > /etc/nginx/conf.d/default.conf

# Set port for DigitalOcean App Platform
EXPOSE 8080

# Start nginx
CMD ["nginx", "-g", "daemon off;"]