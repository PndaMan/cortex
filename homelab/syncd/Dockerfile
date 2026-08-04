# Build stage — small static binary, cached by compose after the first build.
FROM rust:1-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release

FROM alpine:3
COPY --from=build /src/target/release/cortex-syncd /usr/local/bin/cortex-syncd
VOLUME /data
EXPOSE 8787
CMD ["cortex-syncd"]
