ARG IMAGE_TAG=3.21
FROM ghcr.io/linuxserver/baseimage-alpine:${IMAGE_TAG} AS runtime

WORKDIR /app

COPY ./autopulse /bin

HEALTHCHECK --interval=10s --timeout=5s --start-period=5s --retries=3 CMD wget --quiet --tries=1 --spider http://127.0.0.1:${AUTOPULSE__APP__PORT:-2875}/stats || exit 1

CMD ["/usr/bin/with-contenv", "/bin/autopulse"]