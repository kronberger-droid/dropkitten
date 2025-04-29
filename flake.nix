{
  description = "dropkitten helper for Nix flakes";

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
      }
    );
}
