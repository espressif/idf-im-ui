{
  description = "ESP-IDF Installation Manager (EIM) - CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    # Pinned copy of the IDF versions list, embedded at build time (the Nix
    # sandbox has no network). Refresh with: nix flake update idf-versions
    idf-versions = {
      url = "file+https://dl.espressif.com/dl/esp-idf/idf_versions.json";
      flake = false;
    };
  };

  outputs =
    { self, nixpkgs, idf-versions }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});

      cargoToml = builtins.fromTOML (builtins.readFile ./src-tauri/Cargo.toml);

      mkEim =
        pkgs:
        let
          inherit (pkgs) lib stdenv;
          # Tools EIM expects on PATH (mirrors the deb/rpm dependencies).
          # Appended to PATH, so tools the user already has take precedence.
          runtimeTools = with pkgs; [
            git
            wget
            flex
            bison
            gperf
            ccache
            cmake
            ninja
            dfu-util
            # basic userland used by eim's child processes (scripts call sed etc.)
            gnused
            gnugrep
            coreutils
            findutils
            # nixpkgs' bare python3 has no pip; EIM's sanity check needs it
            (python3.withPackages (ps: [ ps.pip ]))
          ];
        in
        pkgs.rustPlatform.buildRustPackage {
          pname = "eim";
          version = cargoToml.package.version;

          src = self;
          # Cargo.toml / Cargo.lock live in src-tauri/ ("source" is the flake store path name).
          sourceRoot = "source/src-tauri";

          cargoLock = {
            lockFile = ./src-tauri/Cargo.lock;
            # git dependencies (lzma-rs, dialoguer, idf-env, ...) are fetched
            # without maintaining outputHashes. Not allowed in nixpkgs itself;
            # replace with explicit outputHashes when upstreaming.
            allowBuiltinFetchGit = true;
          };

          # CLI only: no tauri / webkitgtk.
          buildNoDefaultFeatures = true;
          buildFeatures = [ "cli" ];

          # build.rs reads this instead of curl-ing dl.espressif.com.
          EIM_IDF_VERSIONS_FILE = idf-versions;

          # The `cli` feature enables `vendored-openssl`, which compiles
          # OpenSSL from source (needs perl). openssl/pkg-config are kept as a
          # fallback if that feature is ever dropped.
          nativeBuildInputs = with pkgs; [
            pkg-config
            perl
            cmake # aws-lc-sys (rustls crypto provider) builds from source
            makeWrapper
            installShellFiles
          ];
          buildInputs = [ pkgs.openssl pkgs.zstd ] ++ lib.optionals stdenv.hostPlatform.isDarwin [ pkgs.libiconv ];

          # cmake is only needed by aws-lc-sys's build script, not for configuring eim.
          dontUseCmakeConfigure = true;

          # Newer nixpkgs sets *_USE_SYSTEM / *_NO_VENDOR variables for common
          # -sys crates. For the ones we have no (version-matching) system
          # library for, build the bundled copy instead:
          #  - aws-lc-sys wants a system AWS-LC
          #  - libgit2-sys wants a system libgit2 in the exact range 1.9.7..1.10
          preBuild = ''
            unset AWS_LC_SYS_USE_SYSTEM
            unset LIBGIT2_NO_VENDOR
          '';

          # Tests need network access and a writable HOME with real tools.
          doCheck = false;

          postInstall = ''
            installManPage ../man/eim.1

            installShellCompletion --cmd eim \
              --bash <($out/bin/eim completions bash) \
              --zsh <($out/bin/eim completions zsh) \
              --fish <($out/bin/eim completions fish)

            wrapProgram $out/bin/eim \
              --suffix PATH : ${lib.makeBinPath runtimeTools}
          '';

          meta = {
            description = "ESP-IDF Installation Manager - unified installer for ESP-IDF and its tooling";
            homepage = "https://github.com/espressif/idf-im-ui";
            license = lib.licenses.asl20;
            mainProgram = "eim";
            platforms = lib.platforms.unix;
          };
        };
    in
    {
      packages = forAllSystems (pkgs: rec {
        eim = mkEim pkgs;
        default = eim;
      });

      apps = forAllSystems (pkgs: rec {
        eim = {
          type = "app";
          program = "${self.packages.${pkgs.stdenv.hostPlatform.system}.eim}/bin/eim";
        };
        default = eim;
      });

      overlays.default = final: _prev: { eim = mkEim final; };

      # NixOS module: installs eim and makes the prebuilt (FHS-linked)
      # toolchains that eim downloads runnable via nix-ld.
      #
      #   imports = [ inputs.eim.nixosModules.default ];
      #   programs.eim.enable = true;
      nixosModules.default =
        { config, lib, pkgs, ... }:
        let
          cfg = config.programs.eim;
        in
        {
          options.programs.eim = {
            enable = lib.mkEnableOption "ESP-IDF Installation Manager (eim) and the runtime support its downloaded toolchains need";

            package = lib.mkOption {
              type = lib.types.package;
              default = self.packages.${pkgs.stdenv.hostPlatform.system}.eim;
              defaultText = lib.literalExpression "inputs.eim.packages.\${system}.eim";
              description = "The eim package to install.";
            };

            enableNixLd = lib.mkOption {
              type = lib.types.bool;
              default = true;
              description = ''
                Enable `programs.nix-ld` with the libraries needed by the
                toolchains, openocd and Python wheels that eim downloads.
                Without it these dynamically linked binaries do not start on NixOS.
              '';
            };

            enableQemuLibraries = lib.mkOption {
              type = lib.types.bool;
              default = false;
              description = "Also provide the libraries needed by the QEMU build eim can download.";
            };

            udevRules = lib.mkOption {
              type = lib.types.bool;
              default = true;
              description = ''
                Install the openocd udev rules (debug/flash access without root).
                eim cannot write to /etc/udev/rules.d on NixOS.
              '';
            };
          };

          config = lib.mkIf cfg.enable {
            environment.systemPackages = [ cfg.package ];

            programs.nix-ld = lib.mkIf cfg.enableNixLd {
              enable = true;
              libraries =
                with pkgs;
                [
                  stdenv.cc.cc.lib # libstdc++, libgcc_s
                  zlib
                  libusb1
                  libffi
                  openssl
                  ncurses
                  expat
                  bzip2
                  xz
                  zstd
                  udev
                ]
                ++ lib.optionals cfg.enableQemuLibraries [
                  glib
                  pixman
                  SDL2
                  libslirp
                  libgcrypt
                ];
            };

            services.udev.packages = lib.mkIf cfg.udevRules [ pkgs.openocd ];
          };
        };

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          inputsFrom = [ self.packages.${pkgs.stdenv.hostPlatform.system}.eim ];
          packages = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            rust-analyzer
          ];
          EIM_IDF_VERSIONS_FILE = idf-versions;
        };
      });

      formatter = forAllSystems (pkgs: pkgs.nixfmt-rfc-style);
    };
}
