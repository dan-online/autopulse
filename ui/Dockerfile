FROM public.ecr.aws/docker/library/node:24-alpine AS builder
WORKDIR /app
COPY package.json .
COPY yarn.lock .

RUN corepack enable
RUN yarn config set nodeLinker node-modules
RUN yarn install --immutable

COPY . .

RUN yarn build
RUN rm -rf node_modules
RUN yarn install --production --immutable

FROM public.ecr.aws/docker/library/node:24-alpine
WORKDIR /app
COPY --from=builder /app/build build/
COPY --from=builder /app/node_modules node_modules/
COPY package.json .

ENV PORT=2880

EXPOSE 2880

ENV NODE_ENV=production
CMD [ "node", "build" ]