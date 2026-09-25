NPM := $(CURDIR)/node_modules
LIB := $(CURDIR)/lib/skia.node
LIB_SRC := Cargo.toml lib/prebuild.mjs $(wildcard src/*.rs) $(wildcard src/*/*.rs) $(wildcard src/*/*/*.rs)
LATEST_RELEASE = $(shell gh release list --limit 1 --json tagName,isDraft --jq '.[0] | .tagName + (if .isDraft then " (draft)" else "" end)')
PACKAGE_VERSION = $(shell npm run env | grep npm_package_version | sed -e 's/^.*=/v/')
PRERELEASE_FLAG = $(if $(findstring -,$(PACKAGE_VERSION)),--prerelease)
NPM_VERSION = $(shell npm view skia-canvas version)
CONTAINER_VERSION ?= $(shell date +%Y.%m)
.PHONY: optimized dev test debug visual check clean distclean bump release containers skia-version with-local-skia
.DEFAULT_GOAL := $(LIB)

$(NPM):
	npm ci --ignore-scripts

$(LIB): $(NPM) $(LIB_SRC)
	@npm run build -- dev
	@touch $(LIB)

optimized: $(NPM)
	@rm -f $(LIB)
	@npm run build

dev: $(NPM) $(LIB_SRC)
	@npm run build -- custom
	@touch $(LIB)

test: $(LIB)
	node tests/runner full

debug: $(LIB)
	node tests/runner debug

visual: $(LIB)
	@node --watch-path lib --watch-path tests/visual tests/visual

check:
	cargo check

clean:
	cargo clean -p skia-canvas
	rm -f $(LIB)

distclean: clean
	rm -rf $(NPM)
	cargo clean

bump:
	@if [[ -z "$(version)" ]]; then \
	  printf "Usage: make bump version=<semver>\n       (e.g. make bump version=4.0.0-rc3)\n"; exit 1; fi
	@/bin/echo -n "Update $(PACKAGE_VERSION:v%=%) -> $(version)? [y/N] "
	@read line; if [[ $$line != "y" ]]; then exit 1; fi
	@npm version $(version) --no-git-tag-version
	@sed -i '' -e '1,/^version = /s/^version = .*/version = "$(version)"/' Cargo.toml
	@cargo update -p skia-canvas

release:
	@if [[ `jj diff --from main --to @ package.json` != "" ]]; then \
	  printf "Commit the package.json change onto main first:\n\n"; \
	  jj diff --from main --to @ package.json; exit 1; fi
	@if [[ `jj log --ignore-working-copy --no-graph -r 'main@origin..main' -T 'commit_id ++ "\n"'` != "" ]]; then \
	  printf "Unpushed commits on main:\n"; \
	  jj log --ignore-working-copy --no-graph -r 'main@origin..main' \
	    -T 'commit_id.shortest(9) ++ " " ++ description.first_line() ++ "\n"'; exit 1; fi
	@if [[ `gh release view $(PACKAGE_VERSION) --json isDraft --jq .isDraft 2>/dev/null` == "false" ]]; then \
	  printf "Already published $(PACKAGE_VERSION)\n"; exit 1; fi
	@echo
	@echo "Currently on NPM:  $(NPM_VERSION)"
	@echo "Latest Release:    $(LATEST_RELEASE)"
	@echo "Package Version:   $(PACKAGE_VERSION)"
	@echo
	@/bin/echo -n "Release $(PACKAGE_VERSION) (draft + build)? [y/N] "
	@read line; if [[ $$line != "y" ]]; then exit 1; fi
	@if ! gh release view $(PACKAGE_VERSION) > /dev/null 2>&1; then \
	  gh release create $(PACKAGE_VERSION) $(PRERELEASE_FLAG) --draft --fail-on-no-commits --generate-notes \
	    --target `jj log --ignore-working-copy --no-graph -r main -T commit_id`; fi
	@printf "\nBuilding native binaries for $(PACKAGE_VERSION)\n"
	@gh workflow run build.yml --ref main
	@printf "\nNext: wait for compilation, then publish the release on github to submit to npm\n"

containers:
	@gh workflow run containers.yml -f version=$(CONTAINER_VERSION)
	@printf "\nNext: wait for images, then update the Linux containers in build.yml to 'ghcr.io/{0}-{1}:$(CONTAINER_VERSION)'\n"

# linux-build helpers
skia-version:
	@grep -m 1 '^skia-safe' Cargo.toml | egrep -o '[0-9\.]+'

with-local-skia:
	echo '' >> Cargo.toml
	echo '[patch.crates-io]' >> Cargo.toml
	echo 'skia-safe = { path = "../rust-skia/skia-safe" }' >> Cargo.toml
	echo 'skia-bindings = { path = "../rust-skia/skia-bindings" }' >> Cargo.toml
