FROM --platform=$BUILDPLATFORM ghcr.io/linuxserver/baseimage-alpine:3.20 AS runtime

WORKDIR /app

COPY ./autopulse /bin

ENV S6_AUTOPULSE_DIR=/etc/s6-overlay/s6-rc.d/svc-autopulse

RUN mkdir -p $S6_AUTOPULSE_DIR && \
    echo "#!/usr/bin/with-contenv sh\n# shellcheck shell=sh\n/bin/autopulse" > $S6_AUTOPULSE_DIR/run && \
    chmod +x $S6_AUTOPULSE_DIR/run && \
    echo "longrun" > $S6_AUTOPULSE_DIR/type && \
    echo "3" > $S6_AUTOPULSE_DIR/notification-fd && \
    mkdir $S6_AUTOPULSE_DIR/dependencies.d/ && echo "" > $S6_AUTOPULSE_DIR/dependencies.d/init-services