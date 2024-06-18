## Using the test suite module

```sh
# Navigate tp the project root.
cd contracts

# Build required Docker images locally.
bun run build-images

# Verify the sanity tests succeed
bun test scripts/suite.test.ts --timeout 1200000
```