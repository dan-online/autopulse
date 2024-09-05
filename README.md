[![Forks][forks-shield]][forks-url]
[![Stargazers][stars-shield]][stars-url]
[![Issues][issues-shield]][issues-url]

<br />
<div align="center">
<a href="https://github.com/dan-online/autopulse">
  <img src="assets/logo.webp" alt="Logo" width="80" height="80">
</a>

<h3 align="center">autopulse</h3>
  <p align="center">
    automated scanning tool that bridges media organizers such as Sonarr and Radarr with media servers such as Plex and Jellyfin
    <br />
    <br />
    <a href="https://github.com/dan-online/autopulse/issues">Report Bug</a>
    ·
    <a href="https://github.com/dan-online/autopulse/issues">Request Feature</a>
  </p>
</div>


## About The Project

autopulse is a simple project, designed after the deprecation of [autoscan](https://github.com/Cloudbox/autoscan) and a lot of inspiration comes from there. The goal is to provide a simple, efficient, and reliable way to update your media library reducing full library scans. A key feature is the ability to provide a hash of the file to the API, which will then wait for the file to match that hash before updating targets.

### Terminology

Following autoscan, we use the following terminology:
- **Trigger**: A trigger is a specification for an endpoint that will be called when a file is ready to be processed
  - Manual (default: /triggers/manual)
  - Sonarr
  - Radarr
- **Target**: A target is a specification for a library that will be updated when a file is ready to be processed
  - Plex
  - Jellyfin
  - Command

### Features

- **Rewrites**: rewrites the path provided by the trigger to the path expected by the target
- **Integration**: integrates with Sonarr, Radarr, Plex, Jellyfin, and more in the future
- **Self-Scans**: checks the file exists before updating the target and optionally waits for the file to match a provided hash
- **Reliability**: uses a database to store the state of the scan requests
- **Webhooks**: allow for notifications to be sent when a file is ready to be processed with webhooks such as Discord

## Getting Started

### Docker

The easiest way to get started with autopulse is to use the provided Docker image

> Here is a sample [docker-compose.yml](docker/docker-compose.yml)

```bash
# create a network
$ docker network create autopulse

# postgres database
$ docker run -d --net autopulse --name postgres -e POSTGRES_PASSWORD=autopulse -e POSTGRES_DB=autopulse postgres:alpine

# autopulse
$ docker run -d --net autopulse -e AUTOPULSE__APP__DATABASE_URL=postgres://postgres:autopulse@postgresql/autopulse --name autopulse danonline/autopulse
```

### Configuration

autopulse requires a configuration file to run. By default, it looks for `config.toml` in the current working directory. You can override the [default values](default.toml) using [a config file](example/config.toml) or by [setting environment variables](example/docker-compose.yml) in the format of: ``AUTOPULSE__{SECTION}__{KEY}``. 

For example: `AUTOPULSE__APP__DATABASE_URL`

An example has been provided in the [example](example) directory

> Note: You can provide the config with `json`, `toml`, `yaml`, `json5`, `ron`, or `ini` format

#### Authorization

autopulse uses basic authorization for the API. You can set the username and password in the config file or by setting the `AUTOPULSE__AUTH__USERNAME` and `AUTOPULSE__AUTH__PASSWORD` environment variables.

```toml
[auth]
username = "terry"
password = "yogurt"
```

> **Note**: By default the username and password are `admin` and `password` respectively, however it is HIGHLY recommended to change these values if you are exposing the API to the internet.

#### Examples

```toml
[triggers.sonarr]
type = "sonarr"

[webhooks.discord]
type = "discord"
url = "https://discord.com/api/webhooks/1234567890/abcdefg"

[targets.my_cool_plex]
type = "plex"
url = "http://plex:32400"
token = "<your_token>"

[targets.my_awesome_jellyfin]
type = "jellyfin"
url = "http://jellyfin:8096"
token = "<your_token>"
```

#### Manual

By default a `manual` endpoint is provided which can be used to manually trigger a scan. This can be useful for testing or for when you want to trigger a scan without waiting for a file to be ready.

```bash
$ curl -H 'Authorization: Basic <base_64_encoded_login> -X POST http://localhost:8080/manual?path=/path/to/file&hash=1234567890
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