{
  description = "A development shell for rdm4 project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    flake-parts = {
      url = "github:hercules-ci/flake-parts";
    };

    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    git-hooks-nix = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-parts,
      treefmt-nix,
      git-hooks-nix,
      ...
    }@inputs:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        treefmt-nix.flakeModule
        git-hooks-nix.flakeModule
      ];
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      perSystem =
        {
          pkgs,
          system,
          config,
          ...
        }:
        let
          cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
          gltfValidatorVersion = "2.0.0-dev.3.3";
          gltfValidatorBin = pkgs.stdenvNoCC.mkDerivation {
            pname = "gltf_validator-bin";
            version = gltfValidatorVersion;

            src = pkgs.fetchurl {
              url = "https://github.com/KhronosGroup/glTF-Validator/releases/download/${gltfValidatorVersion}/gltf_validator-${gltfValidatorVersion}-linux64.tar.xz";
              hash = "sha256-+Afr011Gu1E8q4ipIOY6wMM1t33PS5HNjQnqZhszW80=";
            };

            dontConfigure = true;
            dontBuild = true;
            sourceRoot = ".";

            installPhase = ''
              runHook preInstall
              install -Dm755 gltf_validator $out/bin/gltf_validator
              runHook postInstall
            '';
          };
        in
        {
          treefmt = {
            projectRootFile = "flake.nix";
            programs.rustfmt = {
              enable = true;
              edition = "2021";
            };
            programs.nixfmt.enable = true;
            programs.yamlfmt.enable = true;
            programs.actionlint.enable = true;
            programs.mdformat = {
              enable = true;
              plugins = p: [
                p.mdformat-gfm
              ];
            };
          };

          pre-commit = {
            check.enable = true;
            settings = {
              hooks.treefmt = {
                enable = true;
                package = config.treefmt.build.wrapper;
              };
              hooks.clippy = {
                enable = true;
                settings = {
                  allFeatures = true;
                  offline = false;
                };
              };
            };
          };

          packages = {
            gltf_validator = pkgs.buildFHSEnv {
              name = "gltf_validator";
              targetPkgs = pkgs: [
                pkgs.stdenv.cc.cc.lib
              ];
              runScript = "${gltfValidatorBin}/bin/gltf_validator";
            };

            rdm4-bin = pkgs.rustPlatform.buildRustPackage {
              pname = cargoToml.package.name;
              version = cargoToml.package.version;
              src = ./.;
              cargoLock.lockFile = ./Cargo.lock;
            };
            default = config.packages.rdm4-bin;
          };

          devShells.default = pkgs.mkShell {
            name = "rdm4-dev";
            inputsFrom = [ config.pre-commit.devShell ];
            packages = with pkgs; [
              config.treefmt.build.wrapper
              cargo
              rustc
              rust-analyzer
              clippy
              rustfmt
              config.packages.gltf_validator
            ];
            shellHook = ''
              ${config.pre-commit.installationScript}
            '';
          };
        };
    };
}
