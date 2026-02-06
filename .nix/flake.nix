{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
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
              targets = ["wasm32-unknown-unknown" "wasm32-wasip1" "wasm32-wasip2"];
            })
            (rust-bin.selectLatestNightlyWith (toolchain: toolchain.rustfmt))

            sqlx-cli
            glibc
          ];


          # # Provide libclang for flecs
          # LIBCLANG_PATH = lib.makeLibraryPath [ libclang ];

          # # Provide libc for flecs
          # CPATH = lib.makeSearchPathOutput "dev" "include" buildInputs;

          LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;

          DATABASE_URL = "sqlite:games.db";
        };
      }
    );
}
