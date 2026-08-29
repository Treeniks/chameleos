{
  description = "Screen annotation tool for niri and Hyprland";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    crane.url = "github:ipetkov/crane";

    treefmt = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        inputs.treefmt.flakeModule
        inputs.git-hooks.flakeModule
      ];

      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      perSystem =
        {
          inputs',
          config,
          pkgs,
          lib,
          ...
        }:
        let
          components = [
            "cargo"
            "clippy"
            "rust-std"
            "rustc"
            "rustfmt"
          ];
          componentsShell = components ++ [
            "rust-analyzer"
            "rust-src"
          ];

          rustToolchain = inputs'.fenix.packages.stable.withComponents components;
          rustToolchainShell = inputs'.fenix.packages.stable.withComponents componentsShell;

          craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;
          craneLibShell = (inputs.crane.mkLib pkgs).overrideToolchain rustToolchainShell;

          runtimeLibs = [
            pkgs.libGL
            pkgs.vulkan-loader
            pkgs.wayland
          ];
          libraryPath = lib.makeLibraryPath runtimeLibs;

          nativeBuildInputs = [
            pkgs.git
            pkgs.pkg-config
          ];
          buildInputs = [ pkgs.wayland ];

          root = ./.;
          src = lib.fileset.toSource {
            inherit root;
            fileset = lib.fileset.unions [
              (craneLib.fileset.commonCargoSources root)
              (pkgs.lib.fileset.fileFilter (file: file.hasExt "wgsl") root)
            ];
          };

          commonArgs = {
            inherit src;
            # chameleos currently has no tests
            doCheck = false;

            strictDeps = true;
            inherit nativeBuildInputs buildInputs;
          };
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;

          commonArgsDev = commonArgs // {
            CARGO_PROFILE = "dev";
            CARGO_PROFILE_DEV_DEBUG = 0;
          };
          cargoArtifactsDev = craneLib.buildDepsOnly commonArgsDev;

          meta = {
            description = "Screen annotation tool for niri and Hyprland";
            homepage = "https://github.com/Treeniks/chameleos";
            license = lib.licenses.gpl3Plus;
            mainProgram = "chameleos";
            platforms = lib.platforms.linux;
          };

          buildChameleos =
            commonArgs: cargoArtifacts:
            craneLib.buildPackage (
              commonArgs
              // {
                inherit cargoArtifacts;
                postFixup = "patchelf --add-rpath ${libraryPath} $out/bin/chameleos";
                inherit meta;
              }
            );

          chameleos = buildChameleos commonArgs cargoArtifacts;
          # debug build
          chameleos-dev = buildChameleos commonArgsDev cargoArtifactsDev;

          checkArgs = "--workspace --all-targets --all-features";
          docFlags = "--deny warnings";
          docArgs = "--no-deps --document-private-items --workspace --all-features --lib --bins";
        in
        {
          packages = {
            default = config.packages.chameleos;
            inherit chameleos;
            inherit chameleos-dev;
          };

          apps = {
            default = config.apps.chameleos;
            chameleos.program = "${chameleos}/bin/chameleos";
            chamel.program = "${chameleos}/bin/chamel";
          };

          # these checks rely only on the "dev" profile to reduce build times
          checks = {
            inherit chameleos-dev;
            chameleos-clippy = craneLib.cargoClippy (
              commonArgsDev
              // {
                cargoArtifacts = cargoArtifactsDev;
                cargoCheckExtraArgs = checkArgs;
              }
            );
            chameleos-doc = craneLib.cargoDoc (
              commonArgsDev
              // {
                cargoArtifacts = cargoArtifactsDev;
                env.RUSTDOCFLAGS = docFlags;
                cargoDocExtraArgs = docArgs;
              }
            );
          };

          devShells.default = craneLibShell.devShell {
            checks = config.checks;
            shellHook = config.pre-commit.shellHook;
            env.LD_LIBRARY_PATH = libraryPath;

            packages = [ pkgs.cargo-edit ];
          };

          treefmt = {
            projectRootFile = "flake.nix";
            programs = {
              nixfmt.enable = true;
              rustfmt = {
                enable = true;
                package = rustToolchain;
              };
              taplo.enable = true;
              mdformat.enable = true;
            };
          };

          pre-commit = {
            check.enable = false;
            settings.hooks = {
              treefmt.enable = true;
              cargo-check = {
                enable = true;
                # to ensure this hook works even outside a devShell
                # we need to enter the devShell first
                entry = "nix develop --command cargo check ${checkArgs}";
                pass_filenames = false;
              };
            };
          };
        };
    };
}
