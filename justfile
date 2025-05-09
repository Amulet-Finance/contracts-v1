set dotenv-load := true

# build all contracts and generate schemas / TS bindings
dist: dist-clean build-contracts generate-schemas generate-ts

# show all available tasks
menu:
	@just --list

# remove all build artifacts
dist-clean:
	rm -rf arifacts schema ts

# deploy a contract on a network (see scripts/deploy.ts for details)
deploy-contract contract *FLAGS:
	#!/usr/bin/env nu
	bun run scripts/deploy.ts --contract {{contract}} {{FLAGS}}

# check that all rust files are formatted correctly
check-formatting:
	echo "checking formatting"
	cargo fmt --check

# check that there are no linting errors
lint:
	echo "linting"
	cargo clippy

# build neccessary artifacts (e.g. docker images) for the on-chain test suite
setup-on-chain-test-suite:
	echo "setting up on-chain test suite"
	chmod +x scripts/suite/dockerfiles/build-all.sh
	./scripts/suite/dockerfiles/build-all.sh

# fetches the required node modules required by scripts / integration tests
fetch-node-modules:
	bun install

# run a specific on-chain test
on-chain-test test:
	bun test scripts/{{test}}.test.ts --timeout 1600000 --bail 1

# run all on-chain tests
integration-tests:
	#!/usr/bin/env nu
	let tests = (ls scripts | get name | parse -r '^scripts/(?<name>[^/\.]+)\.test(?:\..*)?$');
	for test in $tests {
		echo $"running integration test: $($test.name)"
		just on-chain-test $test.name
		sleep 1sec
	}

# update the expected results for snapshot tests in the specific package
update-snapshots package:
	UPDATE_EXPECT=1 cargo test --package {{package}}

# run the unit tests (see `cargo nextest --help` for options)
unit-tests *NEXTEST_OPTS:
	cargo nextest run {{NEXTEST_OPTS}}

# generate a coverage report for unit-tests (see `cargo llvm-cov --help` for options)
unit-test-coverage *OPTS:
	cargo tarpaulin {{OPTS}}

# ci task pipeline
ci: check-formatting lint unit-tests dist setup-on-chain-test-suite fetch-node-modules integration-tests

update-changelog tag:
	git cliff v1.0.0..HEAD --tag {{tag}} > CHANGELOG.md

# build all contracts and optimize WASM artifacts
build-contracts:
	#!/usr/bin/env nu
	mkdir artifacts;
	# find contract packages
	rg --files contracts --glob Cargo.toml
	| lines
	| par-each {
		open
		| get package.name
		| do {
			# compile wasm artifact
			RUSTFLAGS="-C link-arg=-s" cargo build --package $in --lib --release --target wasm32-unknown-unknown;
			# optimise wasm artifact
			let opt_in = $"target/wasm32-unknown-unknown/release/($in | str replace --all '-' '_').wasm";
			let opt_out = $"artifacts/($in).wasm";
			wasm-opt -Os --signext-lowering $opt_in -o $opt_out;
			$opt_out
		}
	};
	cd artifacts;
	# checksum wasm artifacts
	sha256sum *.wasm | save -f checksum.txt;
	# show files and sizes
	ls ./ | select name size

# generate all the JSON schemas for the contract interfaces
generate-schemas:
	#!/usr/bin/env nu
	mkdir schema;
	let schema_dir = $"(pwd)/schema";
	# find contract packages
	rg --files contracts --glob Cargo.toml
	| lines | each { open }
	| filter { $in.bin? | any { $in.name == "schema" } }
	| par-each {
		get package.name
		| do {
			let tempdir = $"target/contracts/($in)";
			let outdir = $"($schema_dir)/($in)";
			mkdir $tempdir;
			cd $tempdir;
			cargo run --package $in --bin schema;
			rm -rf outdir
			cp -rf ./schema $outdir
		}
	};
	rm -rf target/schemas
	# show result
	ls schema | select name

# generate typescript bindings for all the contract messages
generate-ts:
	#!/usr/bin/env nu
	# find contract packages
	rg --files contracts --glob Cargo.toml
	| lines | each { open }
	| filter { $in.bin? | any { $in.name == "schema" } }
	| par-each {
		get package.name
		| do {
			let contract = ($in | split words | each { str capitalize } | str join);
			let schema_dir = $"./schema/($in)";
			echo $schema_dir
			(bun x @cosmwasm/ts-codegen generate
				--schema $schema_dir
				--out ./ts
				--name $contract
				--no-bundle
				--plugin none)
		}
	};
	# show result
	ls ts | select name
