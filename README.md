# Amulet Finance Contracts

## Deployments

### Mainnet

#### Neutron

| Contract      | Address                                                              |
| ------------- | -------------                                                        |
| `amulet-mint` | `neutron1shwxlkpdjd8h5wdtrykypwd2v62z5glr95yp0etdcspkkjwm5meq82ndxs` |
| `amulet-hub`  | `neutron16d4a7q3wfkkawj4jwyzz6g97xtmj0crkyn06ev74fu4xsgkwnreswzfpcy` |

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
