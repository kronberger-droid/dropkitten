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

          patchPhase = ''
            # prepend the cargo-features line to the top of Cargo.toml
            (echo 'cargo-features = ["edition2024"]' && cat Cargo.toml) > Cargo.toml.tmp
            mv Cargo.toml.tmp Cargo.toml
          '';
          buildPhase = ''
            export RUSTC_BOOTSTRAP=1
            cargo build --release
          '';
          installPhase = ''
            mkdir -p "$out/bin"
            cp "target/release/dropkitten" "$out/bin/dropkitten"
          '';
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
