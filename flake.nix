{
  description = "Example rust project";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem
      (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ self.overlays.default ];
          };
        in
        {
          packages = rec {
            synthtoy = pkgs.synthtoy;
            default = synthtoy;
          };
          checks = self.packages.${system};

          # for debugging
          inherit pkgs;

          devShells.default = pkgs.synthtoy.overrideAttrs (
            old: {
              # make rust-analyzer work
              RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;

              # any dev tools you use in excess of the rust ones
              nativeBuildInputs = old.nativeBuildInputs ++ (
                with pkgs; [
                  rust-analyzer
                  cargo-insta
                ]
              );
            }
          );
        }
      )
    // {
      overlays.default = (
        final: prev:
          let
            inherit (prev) lib;
          in
          {
            synthtoy = final.rustPlatform.buildRustPackage {
              pname = "synthtoy";
              version = "0.1.0";

              cargoLock = {
                lockFile = ./Cargo.lock;
              };

              src = ./.;

              # tools on the builder machine needed to build; e.g. pkg-config
              nativeBuildInputs = with final; [ cmake pkg-config ];

              # native libs
              buildInputs = with final; [ portaudio freetype stdenv expat SDL2 alsa-lib ];
            };
          }
      );
    };
}
