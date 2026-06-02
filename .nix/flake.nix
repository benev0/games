{
  inputs = {
    # nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.11";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ { self, nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
      in {
        devShells.default = with pkgs; mkShell rec {
          buildInputs = [
            (rust-bin.stable.latest.minimal.override {
              extensions = [ "clippy" "rust-analyzer" "rust-docs" "rust-src" ];
              targets = ["wasm32-wasip2"];
            })
            (rust-bin.selectLatestNightlyWith (toolchain: toolchain.rustfmt))

            openssl
            pkg-config
            eza
            fd
            wasm-tools

            sqlx-cli
            glibc
          ];

          LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;

          DATABASE_URL = "sqlite:games.db";
        };
      }
    );
}
