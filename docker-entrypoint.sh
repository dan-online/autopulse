#!/bin/sh
set -e

# Timezone handling
if [ -n "$TZ" ] && [ -f "/usr/share/zoneinfo/$TZ" ]; then
    ln -snf "/usr/share/zoneinfo/$TZ" /etc/localtime
    echo "$TZ" > /etc/timezone
fi

# PUID/PGID remapping - only when running as root
if [ "$(id -u)" = "0" ]; then
    PUID=${PUID:-1000}
    PGID=${PGID:-1000}

    if [ "$PUID" != "0" ]; then
        # Update group if different from default
        if [ "$PGID" != "1000" ]; then
            groupmod -o -g "$PGID" autopulse
        fi

        # Update user if different from default
        if [ "$PUID" != "1000" ]; then
            usermod -o -u "$PUID" autopulse
        fi

        # Fix ownership of config directory
        chown -R autopulse:autopulse /config 2>/dev/null || true

        exec su-exec autopulse "$@"
    fi
fi

# Already non-root or PUID=0 requested - run directly
exec "$@"
