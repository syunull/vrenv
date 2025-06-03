{
  inputs = {
    nixpkgs.url = "https://flakehub.com/f/NixOS/nixpkgs/0.2505.*.tar.gz";
    rust-overlay.url = "github:oxalica/rust-overlay/stable";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      overlays = [
        (import rust-overlay)
        (self: super: { rustToolchain = super.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml; })
      ];

      allSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      forAllSystems =
        f:
        nixpkgs.lib.genAttrs allSystems (system: f { pkgs = import nixpkgs { inherit overlays system; }; });

      cargoToml = nixpkgs.lib.importTOML ./Cargo.toml;
    in
    {
      packages = forAllSystems (
        { pkgs }:
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = cargoToml.package.name;
            version = cargoToml.package.version;
            src = ./.;
            cargoDeps = pkgs.rustPlatform.importCargoLock {
              lockFile = ./Cargo.lock;
            };
          };
        }
      );

      devShells = forAllSystems (
        { pkgs }:
        {
          default = pkgs.mkShell {
            packages =
              (with pkgs; [
                bacon
                go-task
                goreleaser
                pre-commit
                rustToolchain
              ])
              ++ pkgs.lib.optionals pkgs.stdenv.isDarwin (
                with pkgs;
                [
                  libiconv
                ]
              );
          };
        }
      );

      dockerImages = forAllSystems (
        { pkgs }:
        let
          rustBin = self.packages.${pkgs.system}.default;
        in
        {
          default = pkgs.dockerTools.buildImage {
            name = cargoToml.package.name;
            tag = cargoToml.package.version;
            created = "now";

            copyToRoot = pkgs.buildEnv {
              name = "${cargoToml.package.name}";
              paths = [
                rustBin
              ];
              pathsToLink = [ "/bin" ];
            };
            config.Cmd = [ "/bin/vrenv" ];
          };
        }
      );
    };
}
