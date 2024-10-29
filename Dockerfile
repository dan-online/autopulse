FROM --platform=$BUILDPLATFORM alpine AS runtime

ARG TARGET="x86_64-unknown-linux-musl"

WORKDIR /app

COPY target/${TARGET}/release/autopulse /usr/local/bin/

CMD ["/usr/local/bin/autopulse"]