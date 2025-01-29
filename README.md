[![Forks][forks-shield]][forks-url]
[![Stargazers][stars-shield]][stars-url]
[![Issues][issues-shield]][issues-url]

<br />
<div align="center">
<a href="https://github.com/dan-online/autopulse">
  <img src="https://github.com/dan-online/autopulse/raw/main/assets/logo.webp" alt="Logo"
   width="80" height="80">
</a>

<h3 align="center">autopulse</h3>
  <p align="center">
    💫 automated lightweight service that updates media servers like Plex and Jellyfin<br> based on notifications from media organizers like Sonarr and Radarr
    <br />
    <br />
    <a href="https://github.com/dan-online/autopulse/issues">Report Bug</a>
    ·
    <a href="https://github.com/dan-online/autopulse/issues">Request Feature</a>
    .
    <a href="https://autopulse.dancodes.online/">Documentation</a>
    .
    <a href="https://autopulseui.pages.dev/">autopulseUI</a>
  </p>
</div>


## About The Project

autopulse is a web server that receives notifications from media organizers like Sonarr/Radarr/Lidarr/etc ([triggers](#terminology)) and updates the items in media servers like Plex/Jellyfin/Emby/etc ([targets](#terminology)). It is designed to be efficient, only updating the items that have changed, reducing the load on media servers.

> Why migrate from [autoscan](https://github.com/Cloudbox/autoscan)? autoscan is a great project and autopulse takes a lot of inspiration from it, but it is no longer maintained and isn't very efficient at updating libraries as it uses a more general "scan" on a folder rather than a specific file. autopulse finds the corresponding library item and sends an update request directly.

### Terminology

We use the following terminology:
- **Trigger**: A trigger is a specification for an endpoint that will be called when a file is ready to be processed
  - [Manual](#manual) (default: /triggers/manual)
    - Fileflows ([sub-flow](https://github.com/dan-online/autopulse/issues/5#issuecomment-2333917695))
  - Sonarr
  - Radarr
  - Lidarr
  - Readarr
  - Notify
    - Linux: `inotify`
    - MacOS: `FSEvents`
    - Windows: `ReadDirectoryChangesW`
    - Fallback: `polling`
- **Target**: A target is a specification for a library that will be updated when a file is ready to be processed
  - Plex
  - Jellyfin
  - Emby
  - Command
  - Sonarr
  - Radarr
  - Tdarr
  - FileFlows
  - Another autopulse instance

#### Example Flow

1. Sonarr organizes an episode and sends a webhook notification to autopulse
2. autopulse receives the notification and rewrites the path to the expected path for the target
3. autopulse optionally checks the file exists and optionally waits for the file to match a provided hash
4. autopulse sends a request to Plex to update or add the episode information/metadata

### Features

- **Rewrites**: rewrites the path provided by the trigger to the path expected by the target
- **Integration**: integrates with Sonarr, Radarr, Plex, Jellyfin, and more in the future
- **Checks**: checks the file exists before updating the target and optionally waits for the file to match a provided hash
- **Reliability**: uses a database to store the state of the scan requests
- **Webhooks**: allow for notifications to be sent when a file is ready to be processed with webhooks such as Discord
- **User-Interface**: provides a simple web interface to view/add scan requests

## Getting Started

### Docker

The easiest way to get started with autopulse is to use the provided [docker image](https://hub.docker.com/r/danonline/autopulse)

#### Tags

- `latest` - full image with support for postgres/sqlite
- `latest-postgres` - smaller image that only supports Postgres
- `latest-sqlite` - smaller image that only supports SQLite
- `ui` - self-hostable UI for autopulse

##### arm64

Append `-arm64` to the tag to get the arm64 image

- `latest-arm64` - full image with support for postgres/sqlite
- `latest-postgres-arm64` - smaller image that only supports Postgres
- `latest-sqlite-arm64` - smaller image that only supports SQLite

One exception is the `ui` tag which is a joint image for both architectures

- `ui`

#### Compose

> Here is a sample [docker-compose.yml](https://github.com/dan-online/autopulse/blob/main/example/docker-compose.yml)

#### CLI

```bash
# create a network
$ docker network create autopulse

# postgres database
$ docker run -d --net autopulse --name postgres -e POSTGRES_PASSWORD=autopulse -e POSTGRES_DB=autopulse postgres
$ docker run -d --net autopulse -e AUTOPULSE__APP__DATABASE_URL=postgres://postgres:autopulse@postgresql/autopulse --name autopulse danonline/autopulse

# sqlite database
$ docker run -d --net autopulse -e AUTOPULSE__APP__DATABASE_URL=sqlite://database.db --name autopulse danonline/autopulse
# or in-memory
$ docker run -d --net autopulse -e AUTOPULSE__APP__DATABASE_URL=sqlite://:memory: --name autopulse danonline/autopulse
```

### Documentation

All the documentation can be found [on the website](https://autopulse.dancodes.online/)

Here's some quick links:

- [Settings](https://autopulse.dancodes.online/autopulse/settings/)
- [Targets](https://autopulse.dancodes.online/autopulse/service/targets/)
- [Triggers](https://autopulse.dancodes.online/autopulse/service/triggers/)
- [Webhooks](https://autopulse.dancodes.online/autopulse/service/webhooks/)

### Quick Start

#### Configuration

autopulse requires a configuration file to run. By default, it looks for `config.toml` in the current working directory. You can override the default values using a config file or by [setting environment variables](https://github.com/dan-online/autopulse/blob/main/example/docker-compose.yml) in the format of: ``AUTOPULSE__{SECTION}__{KEY}``. 

For example: `AUTOPULSE__APP__DATABASE_URL`

An example has been provided in the [example](https://github.com/dan-online/autopulse/blob/main/example) directory

> Note: You can provide the config with `json`, `toml`, `yaml`, `json5`, `ron`, or `ini` format

##### Authorization

autopulse uses basic authorization for the API. You can set the username and password in the config file or by setting the `AUTOPULSE__AUTH__USERNAME` and `AUTOPULSE__AUTH__PASSWORD` environment variables.

```yaml
auth:
  username: terry
  password: yoghurt
```

> **Note**: By default the username and password are `admin` and `password` respectively, however it is HIGHLY recommended to change these values if you are exposing the API to the internet.

#### Examples

```yaml
triggers:
  my_sonarr:
    type: "sonarr"
    rewrite:
      from: "/downloads"
      to: "/tvshows"

  my_radarr:
    type: "radarr"
    rewrite:
      from: "/downloads"
      to: "/movies"

  my_manual:
    type: "manual"
    rewrite:
      from: "/downloads"
      to: "/"
  
  my_notify:
    type: "notify"
    paths:
      - "/watch"
    rewrite:
      from: "/watch"
      to: "/media"

webhooks:
  my_discord:
    type: "discord"
    url: "https://discord.com/api/webhooks/1234567890/abcdefg"

targets:
  my_plex:
    type: "plex"
    url: "http://plex:32400"
    token: "<your_token>"

  my_different_plex:
    type: "plex"
    url: "http://plex:32401"
    token: "<your_token>"
    rewrite:
      from: "/media"
      to: "/plex"

  my_jellyfin:
    type: "jellyfin"
    url: "http://jellyfin:8096"
    token: "<your_token>"

  my_command:
    type: "command"
    raw: "echo $FILE_PATH >> list.txt"
```


#### Manual

By default a `manual` endpoint is provided which can be used to manually trigger a scan. This can be useful for testing or for when you want to trigger a scan without waiting for a file to be ready.

```bash
$ curl -u 'admin:password' 'http://localhost:2875/triggers/manual?path=/path/to/file&hash=1234567890' 
# or
$ curl -H 'Authorization: Basic <base_64_encoded_login>' 'http://localhost:2875/triggers/manual?path=/path/to/file&hash=1234567890'
```

#### UI

The autopulse ui is a simple web interface that allows you to view and add scan requests. It is available hosted on Cloudflare Pages at [autopulseui.pages.dev](https://autopulseui.pages.dev/) or you can host it yourself using the provided docker image. Note that requests are made server-side so you do not need to expose your autopulse instance to the internet, only the UI when self-hosting.

##### Environment Variables

| Variable | Description | Example |
| --- | --- | --- |
| `FORCE_DEFAULT_SERVER_URL` | Forces the default server URL to be used | `true` |
| `DEFAULT_SERVER_URL` | The default server URL to use | `http://localhost:2875` |
| `FORCE_AUTH` | Forces the UI to use auth from env | `true` |
| `FORCE_SERVER_URL` | Forces the server url | `true` |
| `FORCE_USERNAME` | Forces the username | `true` |
| `FORCE_PASSWORD` | Forces the password | `true` |

###### Examples

Force a default server URL

```env
FORCE_DEFAULT_SERVER_URL=true
DEFAULT_SERVER_URL=http://localhost:2875
```

Force the UI to use the provided auth

```env
FORCE_AUTH=true
FORCE_SERVER_URL=https://localhost:2875
FORCE_USERNAME=admin
FORCE_PASSWORD=password
```

## To-do

- [x] Add more triggers
  - [x] Lidarr
  - [x] Readarr
  - [x] inotify
- [x] Move triggers to structs
- [ ] Hooks
  - [ ] Add/Found/Processed hooks
  - [ ] Move Webhooks to hook
  - [ ] Command hook
- [x] Add more targets
  - [x] Emby
- [ ] Add more webhooks
  - [ ] Generic JSON
- [x] Add more options
  - [x] Cleanup duration (currently 10 days)
  - [x] Jellyfin `metadataRefreshMode` currently set to `FullRefresh`
  - [x] Plex refresh
- [x] Databases
  - [x] SQLite
  - [-] MySQL - linking mysql for alpine docker image is quite complex, so for now not supported unless someone can figure it out
- [x] UI
  - [x] Add/View scan requests
  - [ ] Add/View triggers
  - [ ] Add/View targets
  - [ ] Add/View webhooks

## Contributing

Contributions are what make the open-source community such an amazing place to learn, inspire, and create. Any contributions you make are **greatly appreciated**.

> This project follows the [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) specification

1. Fork the Project
2. Create your Feature Branch (`git checkout -b feat/AmazingFeature`)
3. Commit your Changes (`git commit -m 'feat: add some AmazingFeature'`)
4. Push to the Branch (`git push origin feat/AmazingFeature`)
5. Open a Pull Request

### Development

#### Dependencies

- [Rust](https://www.rust-lang.org/tools/install)

#### Setup

```bash
# clone the repo
$ git clone https://github.com/dan-online/autopulse.git
$ cd autopulse

# basic easy config
$ cat <<EOF > config.toml
[app]
database_url = "sqlite://data/test.sqlite"
log_level = "trace"
EOF

# easy start using vendored/bundled dependencies
$ cargo run --features vendored

# or if you have the dependencies installed (libql-dev, libsqlite3-dev)
$ cargo run
# or if you only have one of the dependencies installed
$ cargo run --no-default-features --features sqlite   # for sqlite
$ cargo run --no-default-features --features postgres # for postgres
```

## License

Distributed under the MIT License. See [`LICENSE`](https://dancodes.mit-license.org/) for more information.

## Contact

DanCodes - <dan@dancodes.online>

Project Link: [https://github.com/dan-online/autopulse](https://github.com/dan-online/autopulse)

[contributors-shield]: https://img.shields.io/github/contributors/dan-online/autopulse.svg?style=for-the-badge
[contributors-url]: https://github.com/dan-online/autopulse/graphs/contributors
[forks-shield]: https://img.shields.io/github/forks/dan-online/autopulse.svg?style=for-the-badge
[forks-url]: https://github.com/dan-online/autopulse/network/members
[stars-shield]: https://img.shields.io/github/stars/dan-online/autopulse.svg?style=for-the-badge
[stars-url]: https://github.com/dan-online/autopulse/stargazers
[issues-shield]: https://img.shields.io/github/issues/dan-online/autopulse.svg?style=for-the-badge
[issues-url]: https://github.com/dan-online/autopulse/issues
