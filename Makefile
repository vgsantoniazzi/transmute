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

publish:
	git tag -a v$(VERSION) -m "Bump version to $(VERSION)"
	git push --tags
	$(coverage)
	rake build
	gem push pkg/transmute-ruby-$(VERSION).gem
