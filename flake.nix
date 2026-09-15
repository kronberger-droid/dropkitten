{
  description = "dropkitten - window drop utility";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      supportedSystems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
      pkgsFor = system: nixpkgs.legacyPackages.${system};
    in {
      packages = forAllSystems (system: {
        dropkitten = (pkgsFor system).rustPlatform.buildRustPackage {
          pname = "dropkitten";
          version = "0.2.0";
          src = self;
          cargoLock = {
            lockFile = ./Cargo.lock;
            # Bump together with Cargo.lock. Fetched at build time, so a
            # rebased niri fork only breaks machines without it in a cache.
            outputHashes."niri-ipc-26.4.0" = "sha256-iVewTIIpZnapeDW+nF+M1F7IPp5Jc8b7lqObSydct74=";
          };
        };

        default = self.packages.${system}.dropkitten;
      });

      devShells = forAllSystems (system:
        let pkgs = pkgsFor system;
        in {
          default = pkgs.mkShell {
            name = "dropkitten-dev";
            buildInputs = with pkgs; [
              cargo
              rustc
              rustfmt
              clippy
              rust-analyzer
              pkg-config
            ];
          };
        });
    };
}
