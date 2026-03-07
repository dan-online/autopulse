{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
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

        devShells.default = pkgs.mkShell {
          packages = [
            rust
            pkgs.cargo-nextest
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
          RUST_SRC_PATH = "${rust}/lib/rustlib/src/rust/library";
        };
      }
    );
}
