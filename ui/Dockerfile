FROM nixos/nix

RUN mkdir -p /etc/nix; echo -e "experimental-features = nix-command flakes\nfilter-syscalls = false" >> /etc/nix/nix.conf

WORKDIR /app

# bring in exact environment definition
COPY flake.nix flake.lock .

# materialize environment so dependencies are cached
RUN nix develop --command echo Loaded Nix Environment

COPY package.json yarn.lock .

RUN nix develop --command bash -c "yarn config set nodeLinker node-modules && yarn install --immutable"

COPY . .

RUN nix develop --command yarn build
RUN rm -rf node_modules
RUN nix develop --command yarn workspaces focus --production

ENV PORT=2880
EXPOSE 2880

ENV NODE_ENV=production
CMD [ "nix", "develop", "--command", "node", "build" ]