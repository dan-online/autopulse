FROM --platform=$BUILDPLATFORM alpine AS runtime

WORKDIR /app

COPY ./autopulse /usr/local/bin/

CMD ["/usr/local/bin/autopulse"]