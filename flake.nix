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
        {
          treefmt = {
            projectRootFile = "flake.nix";
            programs.rustfmt = {
              enable = true;
              edition = "2021";
            };
            programs.nixfmt.enable = true;
            programs.yamlfmt.enable = true;
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
                settings.allFeatures = true;
              };
            };
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
            ];
            shellHook = ''
              ${config.pre-commit.installationScript}
            '';
          };
        };
    };
}
