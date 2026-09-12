{
  self,
  pkgs,
  rust,
  crane,
}:
let
  inherit (pkgs) lib;
  version = (builtins.fromTOML (builtins.readFile ../Cargo.toml)).workspace.package.version;
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../.cargo
      ../src
      ../crates
      ../assets
      ../README.md
    ];
  };
  craneLib = (crane.mkLib pkgs).overrideToolchain rust;
  cargoVendorDir = craneLib.vendorCargoDeps { inherit src; };
  mkBuild =
    packageSet: databaseFeatures:
    let
      static = packageSet.stdenv.hostPlatform.isStatic;
      toolchainFor =
        p:
        (p.rust-bin.fromRustupToolchainFile ../rust-toolchain.toml).override {
          targets = lib.optional static packageSet.stdenv.hostPlatform.rust.rustcTarget;
        };
      toolchain = toolchainFor pkgs;
      builder = (crane.mkLib packageSet).overrideToolchain toolchainFor;
      libpq =
        if static then
          packageSet.libpq.overrideAttrs (old: {
            # Nix puts the archives in dev; pkg-config needs that path to bundle them together.
            postFixup = (old.postFixup or "") + ''
              substituteInPlace "$dev/lib/pkgconfig/libpq.pc" \
                --replace-fail 'libdir=''${exec_prefix}/lib' "libdir=$dev/lib"
            '';
          })
        else
          packageSet.libpq;
      commonArgs = {
        inherit src version cargoVendorDir;
        pname = "autopulse";
        strictDeps = true;
        cargoExtraArgs = "--locked --workspace --no-default-features --features ${lib.concatStringsSep "," databaseFeatures}";
        nativeBuildInputs = [
          pkgs.pkg-config
          pkgs.cmake
        ];
        buildInputs =
          lib.optionals (lib.elem "postgres" databaseFeatures) [ libpq ]
          ++ lib.optionals (lib.elem "sqlite" databaseFeatures) [ packageSet.sqlite ];
        env = {
          # Embedded standard-library source paths must not retain the toolchain.
          RUSTFLAGS = "--remap-path-prefix=${toolchain}=/rustc";
        }
        // lib.optionalAttrs static {
          CARGO_BUILD_TARGET = packageSet.stdenv.hostPlatform.rust.rustcTarget;
          # Include libpq's private dependencies when linking its static archive.
          PKG_CONFIG_ALL_STATIC = "1";
          OPENSSL_NO_VENDOR = "1";
        };
      };
      cargoArtifacts = builder.buildDepsOnly commonArgs;
      appArgs = commonArgs // {
        inherit cargoArtifacts;
        env = commonArgs.env // {
          GIT_REVISION = self.shortRev or self.dirtyShortRev or "unknown";
        };
      };
      testArgs = appArgs // {
        env = appArgs.env // {
          SSL_CERT_FILE = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";
        };
      };
    in
    {
      package = builder.buildPackage (
        appArgs
        // {
          # Nextest and doctests are separate, cacheable checks.
          doCheck = false;
          disallowedReferences = [ toolchain ];
        }
        // lib.optionalAttrs static {
          postFixup = ''
            ${pkgs.binutils}/bin/readelf --program-headers "$out/bin/autopulse" > headers
            ${pkgs.binutils}/bin/readelf --dynamic "$out/bin/autopulse" > dynamic
            ! grep -q INTERP headers
            ! grep -q NEEDED dynamic
          '';
        }
      );
      nextest = builder.cargoNextest (
        testArgs
        // {
          doInstallCargoArtifacts = false;
        }
      );
      doctests = builder.cargoTest (
        testArgs
        // {
          cargoTestExtraArgs = "--doc";
          doInstallCargoArtifacts = false;
        }
      );
      clippy = builder.cargoClippy (
        appArgs
        // {
          cargoClippyExtraArgs = "-- --deny warnings";
          doInstallCargoArtifacts = false;
        }
      );
      database = builder.mkCargoDerivation (
        appArgs
        // {
          pnameSuffix = "-database-check";
          buildPhaseCargoCommand = "cargoWithProfile check --locked -p autopulse-database --no-default-features --features ${lib.concatStringsSep "," databaseFeatures}";
          doCheck = false;
          doInstallCargoArtifacts = false;
        }
      );
      postgresTest = builder.cargoTest (
        testArgs
        // {
          cargoExtraArgs = "--locked -p autopulse-service --no-default-features --features ${lib.concatStringsSep "," databaseFeatures}";
          cargoTestExtraArgs = "--lib postgres_ -- --ignored --test-threads=1";
          doInstallCargoArtifacts = false;
          nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ pkgs.postgresql_17 ];
          preCheck = ''
            export PGDATA="$TMPDIR/postgres"
            export PGHOST="$TMPDIR"
            export PGUSER=postgres
            initdb --username="$PGUSER" --auth=trust --encoding=UTF8 --no-locale
            pg_ctl -l "$TMPDIR/postgres.log" -o "-k $PGHOST -c listen_addresses=" -w start
            stopPostgres() { pg_ctl -m fast -w stop; }
            exitHooks+=(stopPostgres)
            failureHooks+=(stopPostgres)
            createdb autopulse_test
            export AUTOPULSE_TEST_POSTGRES_URL="postgresql:///autopulse_test?host=$PGHOST&user=$PGUSER"
          '';
        }
      );
      docs = builder.cargoDoc (
        appArgs
        // {
          postInstall = ''
            echo '<meta http-equiv="refresh" content="0;url=/autopulse/index.html">' > "$out/share/doc/index.html"
            rm -f "$out/share/doc/.lock"
          '';
        }
      );
    };
  variants = {
    full = mkBuild pkgs [
      "postgres"
      "sqlite"
    ];
    postgres = mkBuild pkgs [ "postgres" ];
    sqlite = mkBuild pkgs [ "sqlite" ];
  };
  staticBuild = mkBuild pkgs.pkgsStatic [
    "postgres"
    "sqlite"
  ];
  mkImage =
    variant: package:
    pkgs.dockerTools.buildLayeredImage {
      name = "autopulse";
      tag = variant;
      contents = [
        pkgs.dockerTools.binSh
        pkgs.dockerTools.usrBinEnv
        pkgs.dockerTools.caCertificates
      ];
      extraCommands = ''
        mkdir -p bin etc usr/share app config data tmp
        ln -s ${package}/bin/autopulse bin/autopulse
        ln -s ${pkgs.tzdata}/share/zoneinfo usr/share/zoneinfo
        cp ${../docker-entrypoint.sh} docker-entrypoint.sh
        chmod 555 docker-entrypoint.sh
        echo 'root:x:0:0:root:/root:/bin/sh' > etc/passwd
        echo 'autopulse:x:1000:1000::/config:/bin/sh' >> etc/passwd
        echo 'root:x:0:' > etc/group
        echo 'autopulse:x:1000:' >> etc/group
      '';
      fakeRootCommands = ''
        chown 1000:1000 app config data
        chmod 1777 tmp
      '';
      config = {
        WorkingDir = "/app";
        Entrypoint = [
          "${pkgs.tini}/bin/tini"
          "--"
          "/docker-entrypoint.sh"
        ];
        Cmd = [ "/bin/autopulse" ];
        Env = [
          "PATH=/bin:/usr/bin:${
            lib.makeBinPath [
              pkgs.coreutils
              pkgs.shadow
              pkgs.su-exec
              pkgs.wget
              pkgs.curl
            ]
          }"
          "TZDIR=/usr/share/zoneinfo"
          "SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt"
        ];
        Healthcheck = {
          Test = [
            "CMD-SHELL"
            "wget -q -O /dev/null --tries=1 http://127.0.0.1:\${AUTOPULSE__APP__PORT:-2875}/stats || exit 1"
          ];
          Interval = 10000000000;
          Timeout = 5000000000;
          StartPeriod = 5000000000;
          Retries = 3;
        };
        Labels = {
          "org.opencontainers.image.title" = "autopulse";
          "org.opencontainers.image.source" = "https://github.com/dan-online/autopulse";
          "org.opencontainers.image.version" = package.version;
          "org.opencontainers.image.revision" = self.rev or self.dirtyRev or "unknown";
        };
      };
    };
in
{
  packages =
    lib.mapAttrs (_: build: build.package) variants
    // lib.mapAttrs' (
      name: build: lib.nameValuePair "image-${name}" (mkImage name build.package)
    ) variants
    // {
      default = variants.full.package;
      image = mkImage "full" variants.full.package;
      docs = variants.full.docs;
      static = staticBuild.package;
    };
  checks =
    lib.foldlAttrs
      (
        acc: name: build:
        acc
        // {
          "build-${name}" = build.package;
          "clippy-${name}" = build.clippy;
          "test-${name}" = build.nextest;
          "doctest-${name}" = build.doctests;
        }
      )
      {
        fmt = craneLib.cargoFmt { inherit src; };
        docs = variants.full.docs;
        postgres-integration = variants.full.postgresTest;
        database-postgres = variants.postgres.database;
        database-sqlite = variants.sqlite.database;
        build-static = staticBuild.package;
        test-static = staticBuild.nextest;
        doctest-static = staticBuild.doctests;
      }
      variants;
}
