ARG PACKAGE=trackage

FROM cgr.dev/chainguard/rust as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM cgr.dev/chainguard/glibc-dynamic
COPY --from=builder --chown=nonroot:nonroot /app/target/release/${PACKAGE} /usr/local/bin/${PACKAGE}
CMD ["/usr/local/bin/trackage"]