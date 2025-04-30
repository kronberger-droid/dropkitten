{ description = "dropkitten helper for Nix flakes with basic Rust dev shell";

inputs = {
  nixpkgs.url     = "github:NixOS/nixpkgs/nixos-24.11";
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

          cargoHash = "sha256-nA6T8soi2vDO4e1Qj/a6TuNr4NkIyhcR3bRjUhvL6gA=";

          patchPhase = ''
            substituteInPlace Cargo.toml \
              --replace 'edition = "2024"' 'cargo-features = ["edition2024"]
              edition = "2024"'
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
