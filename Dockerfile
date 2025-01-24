ARG IMAGE_TAG=3.21
FROM --platform=$TARGETPLATFORM ghcr.io/linuxserver/baseimage-alpine:${IMAGE_TAG} AS runtime

WORKDIR /app

COPY ./autopulse /bin

ENV S6_AUTOPULSE_DIR=/etc/s6-overlay/s6-rc.d/svc-autopulse

HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 CMD wget --quiet --tries=1 --spider http://127.0.0.1:${AUTOPULSE__APP__PORT:-2875}/stats || exit 1

RUN mkdir -p $S6_AUTOPULSE_DIR && \
    echo '#!/usr/bin/with-contenv bash' >> $S6_AUTOPULSE_DIR/run && \
    echo '# shellcheck shell=bash' >> $S6_AUTOPULSE_DIR/run && \
    echo '' >> $S6_AUTOPULSE_DIR/run && \
    echo 'cd /app && /bin/autopulse' >> $S6_AUTOPULSE_DIR/run && \
    chmod +x $S6_AUTOPULSE_DIR/run && \
    echo "longrun" > $S6_AUTOPULSE_DIR/type && \
    echo "3" > $S6_AUTOPULSE_DIR/notification-fd && \
    mkdir $S6_AUTOPULSE_DIR/dependencies.d/ && \
    echo "" > $S6_AUTOPULSE_DIR/dependencies.d/init-services && \
    mkdir -p /etc/s6-overlay/s6-rc.d/user/contents.d && \
    echo "" > /etc/s6-overlay/s6-rc.d/user/contents.d/svc-autopulse