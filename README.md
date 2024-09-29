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
    automated scanning tool that integrates widely-used media management<br /> services with various media servers for seamless media organization
    <br />
    <br />
    <a href="https://github.com/dan-online/autopulse/issues">Report Bug</a>
    ·
    <a href="https://github.com/dan-online/autopulse/issues">Request Feature</a>
    .
    <a href="https://autopulse.pages.dev/">Documentation</a>
  </p>
</div>


## About The Project

autopulse is a simple project, designed after the deprecation of [autoscan](https://github.com/Cloudbox/autoscan) and a lot of inspiration comes from there. The goal is to provide a simple, efficient, and reliable way to update your media library reducing full library scans. A key feature is the ability to provide a hash of the file to the API, which will then wait for the file to match that hash before updating targets.

### Terminology

Following autoscan, we use the following terminology:
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
- **Target**: A target is a specification for a library that will be updated when a file is ready to be processed
  - Plex
  - Jellyfin
  - Emby
  - Command
  - Tdarr
  - FileFlows
  - Another autopulse instance

### Features

- **Rewrites**: rewrites the path provided by the trigger to the path expected by the target
- **Integration**: integrates with Sonarr, Radarr, Plex, Jellyfin, and more in the future
- **Checks**: checks the file exists before updating the target and optionally waits for the file to match a provided hash
- **Reliability**: uses a database to store the state of the scan requests
- **Webhooks**: allow for notifications to be sent when a file is ready to be processed with webhooks such as Discord

## Getting Started

### Docker

The easiest way to get started with autopulse is to use the provided [Docker image](https://hub.docker.com/r/danonline/autopulse)

#### Tags

- `latest` - full image with support for postgres/sqlite
- `latest-postgres` - smaller image that only supports Postgres
- `latest-sqlite` - smaller image that only supports SQLite

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

All the documentation can be found [on the website](https://autopulse.pages.dev/)

Here's some quick links:

- [Settings](https://autopulse.pages.dev/autopulse/utils/settings/)
- [Targets](https://autopulse.pages.dev/autopulse/service/targets/)
- [Triggers](https://autopulse.pages.dev/autopulse/service/triggers/)
- [Webhooks](https://autopulse.pages.dev/autopulse/service/webhooks/)

### Quick Start

#### Configuration

autopulse requires a configuration file to run. By default, it looks for `config.toml` in the current working directory. You can override the [default values](https://github.com/dan-online/autopulse/blob/main/default.toml) using a config file or by [setting environment variables](https://github.com/dan-online/autopulse/blob/main/example/docker-compose.yml) in the format of: ``AUTOPULSE__{SECTION}__{KEY}``. 

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

  my_lidarr:
    type: "lidarr"
    rewrite:
      from: "/downloads"
      to: "/music"
  
  my_readarr:
    type: "readarr"
    rewrite:
      from: "/downloads"
      to: "/books"
  
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
$ curl -u 'admin:password' 'http://localhost:8080/manual?path=/path/to/file&hash=1234567890' 
# or
$ curl -H 'Authorization: Basic <base_64_encoded_login>' 'http://localhost:8080/manual?path=/path/to/file&hash=1234567890'
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
- [ ] Add more options
  - [x] Cleanup duration (currently 10 days)
  - [x] Jellyfin `metadataRefreshMode` currently set to `FullRefresh`
  - [ ] Plex refresh
- [x] Databases
  - [x] SQLite
  - [-] MySQL - linking mysql for alpine docker image is quite complex, so for now not supported unless someone can figure it out

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
