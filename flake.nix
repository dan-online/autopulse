{
  inputs = {
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
        rust = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
          ];
          targets = [
            "x86_64-unknown-linux-gnu"
            # "x86_64-unknown-linux-musl"
            # "aarch64-unknown-linux-gnu"
            # "aarch64-unknown-linux-musl"
          ];
        };

        features = [
          "postgres"
          "sqlite"
          # "mysql"
        ];
      in
      {
        formatter = pkgs.nixfmt-tree;

        devShells.default = pkgs.mkShell {
          buildInputs = [
            pkgs.openssl
            pkgs.pkg-config
            pkgs.clang
            pkgs.cmake
            pkgs.llvmPackages.clang
            pkgs.llvmPackages.libclang
            pkgs.llvmPackages.libcxxClang

            pkgs.cargo-nextest
            rust

            pkgs.nixfmt
          ]
          ++ pkgs.lib.optionals (pkgs.lib.elem "postgres" features) [
            pkgs.postgresql
          ]
          ++ pkgs.lib.optionals (pkgs.lib.elem "sqlite" features) [
            pkgs.sqlite
          ]
          ++ pkgs.lib.optionals (pkgs.lib.elem "mysql" features) [
            pkgs.libmysqlclient
            pkgs.ncurses
          ]
          ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];

          shellHook = ''
            if [ "$(whoami)" = "dan" ]; then
              export CARGO_TARGET_DIR="$HOME/.cargo/target";
              export RUST_SRC_PATH="$(rustc --print sysroot)/lib/rustlib/src/rust/src";
              export PATH="$PATH:$HOME/.cargo/bin"

              mkdir -p $HOME/.cargo/bin

              echo "cargo clippy --all --fix --allow-staged --no-deps -- -W clippy::all -W clippy::nursery -D warnings && cargo fmt" > $HOME/.cargo/bin/nursery

              chmod +x $HOME/.cargo/bin/nursery
            fi;
          '';

          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          BINDGEN_EXTRA_CLANG_ARGS = "-isystem ${pkgs.llvmPackages.libclang.lib}/lib/clang/${pkgs.lib.getVersion pkgs.clang}/include";
        };
      }
    );
}
