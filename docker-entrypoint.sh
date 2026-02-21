#!/bin/sh
set -e

# Timezone handling
# The TZ env var is the primary mechanism (chrono reads it directly).
# The /etc/localtime symlink is a fallback for tools like `date`.
# Use || true so non-root containers (--user flag) don't crash.
if [ -n "$TZ" ] && [ -f "/usr/share/zoneinfo/$TZ" ]; then
    { ln -snf "/usr/share/zoneinfo/$TZ" /etc/localtime; } 2>/dev/null || true
    { echo "$TZ" > /etc/timezone; } 2>/dev/null || true
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

        # Fix ownership of app and config directories
        chown -R autopulse:autopulse /config /app 2>/dev/null || true

        # Set user environment variables that su-exec doesn't provide
        # Use env to ensure these are passed through exec
        exec env HOME=/config USER=autopulse su-exec autopulse "$@"
    fi
fi

# Already non-root or PUID=0 requested - run directly
exec "$@"
