{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";

    flake-utils.url = "github:numtide/flake-utils";

    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      crane,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        lib = pkgs.lib;

        llvmPackages = pkgs.llvmPackages;

        rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        build = import ./nix/build.nix {
          inherit
            self
            pkgs
            rust
            crane
            ;
        };

        formatterPackage = pkgs.nixfmt-tree;

        features = [
          "postgres"
          "sqlite"
          # "mysql"
        ];

        databasePackages =
          lib.optionals (lib.elem "postgres" features) [ pkgs.libpq ]
          ++ lib.optionals (lib.elem "sqlite" features) [
            pkgs.sqlite
          ]
          ++ lib.optionals (lib.elem "mysql" features) [
            pkgs.libmysqlclient
            pkgs.ncurses
          ];

      in
      {
        formatter = formatterPackage;

        packages = lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux build.packages;
        checks = lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux build.checks;

        devShells.default = pkgs.mkShell {
          packages = [
            rust
            pkgs.cargo-nextest
            pkgs.lefthook
            pkgs.sccache
            formatterPackage
          ];

          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.cmake
            llvmPackages.clang
            llvmPackages.libclang
          ];

          buildInputs = [
            pkgs.openssl
          ]
          ++ databasePackages
          ++ lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];

          RUSTC_WRAPPER = "${pkgs.sccache}/bin/sccache";
          SCCACHE_CLIENT_SIDE = "1";
          RUST_SRC_PATH = "${rust}/lib/rustlib/src/rust/library";
        };
      }
    );
}
