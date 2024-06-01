set dotenv-load := true

dist: dist-clean build-contracts generate-schemas generate-ts

dist-clean:
	rm -rf arifacts schema ts

on-chain-test test:
	bun test scripts/{{test}}.test.ts --timeout 1600000

deploy-contract contract init_msg *FLAGS:
	bun run scripts/deploy.ts --contract {{contract}} --msg '{{init_msg}}' {{FLAGS}}

update-expect package:
	UPDATE_EXPECT=1 cargo test --package {{package}}

test *NEXTEST_OPTS:
	cargo nextest run {{NEXTEST_OPTS}}

test-coverage *LLVM_COV_OPTS:
	cargo llvm-cov {{LLVM_COV_OPTS}}

build-contracts:
	#!/usr/bin/env nu
	do {
		mkdir artifacts
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
	}

generate-schemas:
	#!/usr/bin/env nu
	do {
		mkdir schema;
		let schema_dir = $"(pwd)/schema";
		# find contract packages
		rg --files contracts --glob Cargo.toml
		| lines 
		| par-each { 
			open 
			| get package.name 
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
	}

generate-ts:
	#!/usr/bin/env nu
	do {
		# find contract packages
		rg --files contracts --glob Cargo.toml
		| lines 
		| par-each { 
			open 
			| get package.name 
			| do {
				let contract = ($in | split words | each { str capitalize } | str join);
				let schema_dir = $"./schema/($in)";
				echo $schema_dir
				(bun x cosmwasm-ts-codegen generate 
					--schema $schema_dir 
					--out ./ts 
					--name $contract 
					--no-bundle 
					--plugin none)
			}
		};
		# show result
		ls ts | select name
	}
