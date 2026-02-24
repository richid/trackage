ARG PACKAGE=trackage

FROM cgr.dev/chainguard/rust:latest-dev as builder
USER root
RUN apk add --no-cache openssl-dev pkgconf
USER nonroot
WORKDIR /app
COPY --chown=nonroot:nonroot . .
RUN cargo build --release

FROM cgr.dev/chainguard/glibc-dynamic
COPY --from=builder --chown=nonroot:nonroot /app/target/release/${PACKAGE} /usr/local/bin/${PACKAGE}
CMD ["/usr/local/bin/trackage"]