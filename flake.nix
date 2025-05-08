{ description = "dropkitten helper for Nix flakes with basic Rust dev shell";

inputs = {
  nixpkgs.url     = "github:NixOS/nixpkgs/nixos-unstable";
  flake-utils.url = "github:numtide/flake-utils";
};

outputs = { self, nixpkgs, flake-utils, ... }:
  flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import nixpkgs { inherit system; };
    in {
      packages = {
        dropkitten = pkgs.rustPlatform.buildRustPackage {
          pname     = "dropkitten";
          version   = "0.1.0";
          src       = self;

          cargoHash = "sha256-OoiHm9ZAXYhcpxcYWkcabBHzJlQIcwkhuGPAb2b5H/A=";
        };
      };

      defaultPackage = self.packages.dropkitten;

      devShells = {
        default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustup
            rustc
            cargo
            rustfmt
            clippy
            rust-analyzer
            nushell
          ];

          shellHook = ''
            if ! rustup toolchain list | grep -q stable; then
              rustup toolchain install stable
            fi
            rustup default stable

            export RUSTC_BOOTSTRAP=1

            nu
          '';
        };
      };
    }
  );
}
