## [1.0.3] - 2024-10-26

### Chore

- Bump contract crate versions for vaults
- Artifacts

### Fix

- Unbond total batch amount

### Test

- Multiple redemptions in one batch amounts

## [1.0.2] - 2024-10-06

### Add

- Deposit-cap-proxy: allow configuration per vault & use amulet-core admin scheme

### Build

- Add git-cliff to flake & update-changelog recipe

### Chore

- Bump vault contract crate versions
- Artifacts

### Fix

- Amulet-remote-pos pending batch start hint timestamp
- Active-unbondings query

### Test

- Unit test for pending batch start hint timestamp
- Active-unbondings query

## [1.0.1] - 2024-09-26

### Add

- Stride-redemption-rate-oracle-proxy contract
- Deprecated-contract

### Build

- Speed up neutron-query-relayer docker build with shallow clone
- Rename / add just recipes for ci
- Add ci devshell to flake
- Add ci github action for PRs
- Add comments to just recipes & menu task
- Fix ci action def
- Fix remote-pos & stride-oracle-proxy contract versions

### Chore

- Update readme deployment table
- Artifacts

### Fix

- *(remote-pos-vault)* Limits on number of validators per delegations icq

### Refactor

- On-chain tests

### Test

- Migrate all on-chain tests to use cosmopark suite

### Tests

- Rename and stabilise remote-pos-vault on-chain test
- Rename suite sanity tests so it is differentiated from protocol tests
- *(pos-reconcile-fsm)* Add tests for collect rewards with fee applied

