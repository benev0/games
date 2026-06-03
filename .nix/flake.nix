{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    # nixpkgs.url = "github:nixos/nixpkgs/nixos-26.05";
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
        rust = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "clippy"
            "rust-analyzer"
            "rust-docs"
            "rust-src"
          ];
          targets = ["wasm32-wasip2"];
        };
      in {
        devShells.default = with pkgs; mkShell rec {
          buildInputs = [
            rust

            openssl
            pkg-config
            eza
            fd
            wasm-tools

            sqlx-cli
            # glibc
          ];

          LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;
          RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;

          DATABASE_URL = "sqlite:games.db";
        };
      }
    );
}
