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
        # A basic Rust development shell with toolchain and common tools
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
            # Ensure the stable toolchain is installed and set
            if ! rustup toolchain list | grep -q stable; then
              rustup toolchain install stable
            fi
            rustup default stable

            # Allow using RUSTC_BOOTSTRAP in this shell
            export RUSTC_BOOTSTRAP=1

            nu
          '';
        };
      };
    }
  );
}
