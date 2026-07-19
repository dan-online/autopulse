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
  - A-Train
- **Target**: A target is a specification for a library that will be updated when a file is ready to be processed
  - Plex
  - Jellyfin
  - Emby
  - Command
  - Sonarr
  - Radarr
  - Tdarr
  - FileFlows
  - Audiobookshelf
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
- **Webhooks**: allow for notifications to be sent when a file is ready to be processed with Discord, Matrix Hookshot, or generic JSON webhooks
- **User-Interface**: provides a simple web interface to view/add scan requests

## Getting Started

### Docker

The easiest way to get started with autopulse is to use the provided docker image on [ghcr.io](https://github.com/dan-online/autopulse/pkgs/container/autopulse) or [Docker Hub](https://hub.docker.com/r/danonline/autopulse)

#### Tags

- `latest` - full image with support for postgres/sqlite
- `latest-postgres` - smaller image that only supports Postgres
- `latest-sqlite` - smaller image that only supports SQLite
- `stable` - latest versioned release

> All images are multi-arch and support `linux/amd64`, `linux/arm64`, however -amd64 and -arm64 suffixes can be used to specify the architecture

#### Unraid

An Unraid Community Apps template lives in [`unraid/autopulse.xml`](unraid/autopulse.xml).

Once autopulse is listed in Community Apps, open the **Apps** tab in Unraid, search for `autopulse`, and click **Install**. Until then, install it as a private Community App:

1. Make sure Community Applications is installed (it ships with Unraid via the Apps tab).
2. SSH to your Unraid server and run:

   ```bash
   mkdir -p /boot/config/plugins/community.applications/private/autopulse
   wget -O /boot/config/plugins/community.applications/private/autopulse/autopulse.xml \
     https://raw.githubusercontent.com/dan-online/autopulse/main/unraid/autopulse.xml
   ```

3. In the WebUI, open **Apps** and select **Private apps** from the left sidebar, then click **Install** on autopulse.
4. Adjust the **Media share** path and change **AUTOPULSE__AUTH__PASSWORD** from the default `change-me` before clicking Apply.

#### Compose

Docker Compose files for both SQLite and Postgres are provided in the [example](https://github.com/dan-online/autopulse/blob/main/example)

#### CLI

```bash
# create a network
$ docker network create autopulse

# postgres database
$ docker run -d --net autopulse --name postgres -e POSTGRES_PASSWORD=autopulse -e POSTGRES_DB=autopulse postgres
$ docker run -d --net autopulse -e AUTOPULSE__APP__DATABASE_URL=postgres://postgres:autopulse@postgresql/autopulse --name autopulse ghcr.io/dan-online/autopulse

# sqlite database
$ docker run -d --net autopulse -e AUTOPULSE__APP__DATABASE_URL=sqlite://database.db --name autopulse ghcr.io/dan-online/autopulse
# or in-memory
$ docker run -d --net autopulse -e AUTOPULSE__APP__DATABASE_URL=sqlite://:memory: --name autopulse ghcr.io/dan-online/autopulse
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

autopulse requires a configuration file to run. By default, it searches the current working directory for `config.toml`, `config.yaml`, `config.yml`, or `config.json` in that order. You can pass `--config /path/to/config.toml` to load an explicit file, and override values by [setting environment variables](https://github.com/dan-online/autopulse/blob/main/example/docker-compose.yml) in the format of: ``AUTOPULSE__{SECTION}__{KEY}``.

For example: `AUTOPULSE__APP__DATABASE_URL`

An example has been provided in the [example](https://github.com/dan-online/autopulse/blob/main/example) directory

> Note: You can provide the config as `config.toml`, `config.yaml`, `config.yml`, or `config.json`

> Note: You can also provide the path to a variable by appending `__FILE`
> For example: `AUTOPULSE__AUTH__PASSWORD__FILE=/run/secrets/autopulse_password`

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
    filter:
      exclude:
        - "^/tvshows/extras/"

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

  my_discord_with_mentions:
    type: "discord"
    url: "https://discord.com/api/webhooks/1234567890/abcdefg"
    mentions:
      - targets:
          - here
          - role: "1234567890"
          - user: "9876543210"
        on: [failed, hash_mismatch]
      - targets: [everyone]

  my_hookshot:
    type: "hookshot"
    url: "https://matrix.example.com/_matrix/hookshot/webhook/abcdefg"

  my_json:
    type: "json"
    url: "https://example.com/webhooks/autopulse"

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

  my_audiobookshelf:
    type: "audiobookshelf"
    url: "http://audiobookshelf:13378"
    token: "<your_token>"
    filter:
      include:
        - "^/audiobooks/"
      exclude:
        - "/samples/"

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


#### Configuration Template API

autopulse provides a configuration template API that allows external applications to dynamically generate configurations without embedding static TOML files. This is useful for applications like Bazarr that need to configure autopulse programmatically.

##### GET /api/config-template

Returns configuration templates with optional parameters:

```bash
# Get basic templates
$ curl -u "admin:password" "http://localhost:2875/api/config-template"

# Get templates with specific types
$ curl -u "admin:password" "http://localhost:2875/api/config-template?database=postgres&triggers=sonarr,radarr&targets=plex,jellyfin&output=json"
```

**Query Parameters:**
- `database`: Database type (`sqlite`, `postgres`)
- `triggers`: Comma-separated trigger types (`manual`, `sonarr`, `radarr`, etc)
- `targets`: Comma-separated target types (`plex`, `jellyfin`, `emby`, etc)
- `output`: Output format (`json`, `toml`)

#### UI

The web UI ships in the main autopulse image and is served at `/ui/*` on the same port (default `2875`). It lets you browse scan events, retry failures, view config, and submit manual scans.

Default credentials are `admin` / `password` (the same as the API auth). Change them via the standard `auth.username` / `auth.password` config keys; sessions issued under old credentials are invalidated automatically.

##### Reverse Proxy

To serve the UI behind a reverse proxy with a path prefix, set `app.base_path` and have the proxy pass the prefix through (no strip-prefix). UI routes mount under `base_path` server-side. See [`app` settings](https://autopulse.dancodes.online/autopulse_service/settings/app/struct.App.html) for the full list of relevant options.

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

> If you're a nix user then you can use the provided flake.nix to get started

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

## FAQ

### What URL do I put in Sonarr, Radarr, Lidarr, or Readarr?

Create a webhook/connection in the source app that points at the matching autopulse trigger:

```text
http://<autopulse-host>:2875/triggers/<trigger-name>
```

For example, a config entry named `triggers.radarr` uses `/triggers/radarr`; `triggers.my_radarr` uses `/triggers/my_radarr`. Use `POST` and the same basic auth credentials configured under `auth.username` and `auth.password`.

Enable events that include file paths, such as import/download, upgrade, rename, and delete.

### Which paths should I use for `rewrite.from` and `rewrite.to`?

Use `rewrite` only when the path sent by the trigger is not the path autopulse or a target should use.

- `from` is the path pattern in the webhook payload from Sonarr, Radarr, Lidarr, Readarr, etc.
- `to` is the path autopulse should process next.

For Arr triggers, this usually means the final media path from the Arr app, not the temporary downloader folder. If the source app and target already use the same path, omit `rewrite`.

Trigger rewrites run before the event is stored. Target rewrites run later per target, which is useful when Plex, Jellyfin, Emby, or another target sees the same library through a different mount path. The `from` value is a regex, so anchor it, for example `^/downloads`, when you only want to replace a prefix.

### Does autopulse need my media files mounted inside the container?

Not by default. With `opts.check_path = false`, autopulse can route webhook paths without reading the file itself.

If `opts.check_path = true`, the rewritten path must exist inside the autopulse runtime/container before the event is sent to targets. Hash checks and `anchors` also require filesystem access. Mount those paths read-only if possible, and make sure your rewrites point to the path as autopulse sees it.

Targets still need their own valid library paths and autopulse must be able to reach the target API URL.

### Why did autopulse start with no targets or only the default manual trigger?

That usually means autopulse did not load your config file. Without `--config`, it searches the current working directory for `config.toml`, `config.yaml`, `config.yml`, then `config.json`.

For Docker, mount the file where autopulse expects it, for example `./config.yaml:/app/config.yaml`. Accidentally mounting a directory or mounting the file somewhere else will leave autopulse running with defaults and environment overrides only. Check startup logs for `loaded config from ...` or `no config file found ...`.

### Which Docker tag or binary release should I use?

Use `stable` for the latest versioned release. Use `latest` if you want the newest Docker build from the main branch. Binaries are published with versioned releases and can lag behind Docker `latest`.

Use the default image if you want both SQLite and Postgres support, or the smaller `-sqlite` / `-postgres` tags if you only need one database. SQLite is simplest for most single-instance home installs. Postgres is better if you already run it, expect heavier concurrency, or want a more traditional server database. `sqlite://:memory:` can be used for disposable databases in testing or ephemeral use.

### Why use autopulse instead of Jellyfin's built-in real-time monitoring?

Jellyfin's library monitor (`Library/Watcher` in Emby parlance) watches the filesystem for changes through the host OS. That works well when:

- Your media lives on a local disk.
- File events are reliable, such as on local ext4, btrfs, or NTFS volumes.
- A single Jellyfin instance is your only consumer of the library.

autopulse is useful when those assumptions stop holding:

- **Network shares:** SMB, NFS, and rclone mounts can drop or never emit filesystem events. autopulse uses a push signal from the application that produced the file, such as Sonarr or Radarr.
- **Multi-server fan-out:** one trigger can update Plex, Jellyfin, Emby, multiple Plex servers, or another autopulse instance.
- **Targeted updates:** Jellyfin's monitor starts a library scan. autopulse sends a file-scoped refresh, item-level when `refresh_metadata` is enabled, otherwise a path-scoped notification.
- **Hashes and waits:** autopulse can wait for a file to exist, verify a provided sha256 hash, and delay processing while post-processing scripts finish renames or remuxes.
- **Retries and audit trail:** a temporarily offline target produces a retried event instead of a lost notification. Every event is stored in the database for inspection.

If you only run Jellyfin on local media and filesystem events are reliable, the built-in monitor is fine. autopulse is for the cases where that stops being true.

### Why not just trigger a full library scan on every event?

Full scans are expensive on large libraries. autopulse locates the specific item in the target and refreshes only that item, so the work scales with the size of the change instead of the size of the library.

### Why migrate from autoscan?

[autoscan](https://github.com/Cloudbox/autoscan) is a great project and autopulse takes inspiration from it, but autoscan is no longer maintained. It also uses a general folder scan rather than a specific file update. autopulse finds the corresponding library item and sends an update request directly.

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
