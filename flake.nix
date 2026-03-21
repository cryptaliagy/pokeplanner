{
  description = "PokePlanner – Rust workspace with REST, gRPC, and CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.protobuf     # protoc for tonic-build
            pkgs.buf          # buf CLI for proto management
            pkgs.pkg-config
            pkgs.openssl
          ];

          PROTOC = "${pkgs.protobuf}/bin/protoc";
        };

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "pokeplanner";
          version = "0.1.0";

          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = [
            pkgs.protobuf
            pkgs.pkg-config
          ];

          buildInputs = [
            pkgs.openssl
          ];

          PROTOC = "${pkgs.protobuf}/bin/protoc";

          meta = {
            description = "PokePlanner – Pokémon team planning service";
            license = pkgs.lib.licenses.mit;
          };
        };
      }
    );
}
