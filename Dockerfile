FROM --platform=$BUILDPLATFORM ghcr.io/linuxserver/baseimage-alpine:3.20 AS runtime

WORKDIR /app

COPY ./autopulse /usr/local/bin/

CMD ["/usr/local/bin/autopulse"]