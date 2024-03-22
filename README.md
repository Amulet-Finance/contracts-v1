# Amulet Finance Contracts

## Building

### Prerequisites

If you have Nix installed, or do not mind [installing](https://github.com/DeterminateSystems/nix-installer) it then simply run:

```shell
$ nix develop
```

Otherwise, make sure you have the packages specified in `flake.nix` installed and in your `PATH`.

### Build

```shell
$ just
```

This will:
- Build and optimise WASM contract bytecode, found in `artifacts/`
- Generate JSON schemas for each contract interface, found in `shema/<contract>/`
- Generate Typescript bindings for each contract's message types, found in `ts/`
