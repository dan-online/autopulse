ARG IMAGE_TAG=3.23
FROM alpine:${IMAGE_TAG} AS runtime

RUN apk add --no-cache \
    su-exec \
    tini \
    tzdata \
    shadow \
    wget

RUN addgroup -g 1000 autopulse && \
    adduser -D -u 1000 -G autopulse -h /config autopulse && \
    mkdir -p /app && \
    chown autopulse:autopulse /app /config

WORKDIR /app

COPY ./autopulse /bin/autopulse
COPY ./docker-entrypoint.sh /docker-entrypoint.sh
RUN chmod +x /docker-entrypoint.sh

HEALTHCHECK --interval=10s --timeout=5s --start-period=5s --retries=3 \
# wget -q -O /dev/null --timeout=5 http://127.0.0.1:2875/stats
    CMD wget -q -O /dev/null --tries=1 http://127.0.0.1:${AUTOPULSE__APP__PORT:-2875}/stats || exit 1

ENTRYPOINT ["/sbin/tini", "--", "/docker-entrypoint.sh"]
CMD ["/bin/autopulse"]
