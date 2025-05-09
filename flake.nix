{
  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };
  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
      ];
      perSystem =
        {
          pkgs,
          system,
          ...
        }:
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ inputs.rust-overlay.overlays.default ];
          };

          devShells =
            let
              toolchain = pkgs.rust-bin.fromRustupToolchainFile ./toolchain.toml;
              requiredPkgs = [
                toolchain
                pkgs.just
                pkgs.nushell
                pkgs.ripgrep
                pkgs.binaryen
                pkgs.coreutils
                pkgs.bun
                pkgs.cargo-tarpaulin
                pkgs.cargo-nextest
                pkgs.jq
                pkgs.docker-compose
                pkgs.go
                pkgs.git-cliff
              ];
            in
            {
              default = pkgs.mkShell {
                packages = requiredPkgs ++ [
                  # nice to have
                  pkgs.cargo-audit
                  pkgs.cargo-edit
                  pkgs.nodePackages_latest.typescript-language-server
                  pkgs.nodePackages_latest.prettier
                  pkgs.rust-analyzer-unwrapped
                  pkgs.gopls
                  pkgs.tokei
                  pkgs.lazydocker
                ];
                RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
              };
              ci = pkgs.mkShell {
                packages = requiredPkgs;
              };
            };
        };
    };
}
