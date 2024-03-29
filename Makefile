.ONESHELL:
.SILENT:

VERSION :=`cat $(PWD)/VERSION`

ENGINE_DIR = $(PWD)/engine
COVERAGE_DIR = $(PWD)/coverage/rspec

engine=cd $(ENGINE_DIR)
coverage=cd $(COVERAGE_DIR)

build:
	$(engine)
	cargo build

release:
	$(engine)
	cargo build --target x86_64-unknown-linux-musl --release

test:
	$(engine)
	cargo test

format:
	$(engine)
	cargo fmt

run.dummy:
	$(engine)
	cargo run -- \
	  --files "tests/fixtures/app/**/*.rb" \
	  --coverage "transmute.json" \
	  --command "rspec {file}" \
	  --formatter "html" \
	  --log-level "trace"

generate.coverage:
	$(engine)
	rm -rf transmute.json || true
	COVERAGE=true rspec tests/fixtures/spec/

publish:
	git tag -a v$(VERSION) -m "Bump version to $(VERSION)"
	git push --tags
	$(coverage)
	rake build
	gem push pkg/transmute-ruby-$(VERSION).gem
