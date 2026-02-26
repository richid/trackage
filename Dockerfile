ARG PACKAGE=trackage

FROM cgr.dev/chainguard/rust:latest-dev AS builder
USER root
RUN mkdir -p /config && chown nonroot:nonroot /config
USER nonroot
WORKDIR /app
COPY --chown=nonroot:nonroot . .
RUN cargo build --release

FROM cgr.dev/chainguard/glibc-dynamic
ARG PACKAGE=trackage
COPY --from=builder --chown=nonroot:nonroot /app/target/release/${PACKAGE} /usr/local/bin/${PACKAGE}
COPY --from=builder --chown=nonroot:nonroot /config /config

VOLUME ["/config"]
WORKDIR /config
EXPOSE 3000
CMD ["/usr/local/bin/trackage"]
