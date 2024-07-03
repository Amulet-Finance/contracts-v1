{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
  }: let
    system = "x86_64-linux";

    pkgs = import nixpkgs {
      inherit system;
      overlays = [rust-overlay.overlays.default];
    };

    toolchain = pkgs.rust-bin.fromRustupToolchainFile ./toolchain.toml;
  in {
    devShells.${system}.default = pkgs.mkShell {
      packages = [
        # required
        toolchain
        pkgs.just
        pkgs.nushell
        pkgs.ripgrep
        pkgs.binaryen
        pkgs.coreutils
        pkgs.bun
        pkgs.cargo-llvm-cov
        pkgs.cargo-nextest
        pkgs.jq
        pkgs.docker-compose
        pkgs.go
        # nice to have
        pkgs.cargo-audit
        pkgs.nodePackages_latest.typescript-language-server
        pkgs.nodePackages_latest.prettier
        pkgs.rust-analyzer-unwrapped
        pkgs.gopls
        pkgs.tokei
        pkgs.lazydocker
      ];

      RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";    
    };
  };

  nixConfig = {
    extra-substituters = [
      "https://nix-community.cachix.org"
    ];

    extra-trusted-public-keys = [
      "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
    ];
  };
}
