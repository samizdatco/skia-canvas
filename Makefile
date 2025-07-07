NAPI_VERSION := 8
NPM := $(CURDIR)/node_modules
NODEMON := $(CURDIR)/node_modules/.bin/nodemon
JEST := $(CURDIR)/node_modules/.bin/jest
LIBDIR := $(CURDIR)/lib/v$(NAPI_VERSION)
LIB := $(LIBDIR)/index.node
LIB_SRC := Cargo.toml $(wildcard src/*.rs) $(wildcard src/*/*.rs) $(wildcard src/*/*/*.rs)
GIT_TAG = $(shell git describe)
PACKAGE_VERSION = $(shell npm run env | grep npm_package_version | cut -d '=' -f 2)
NPM_VERSION = $(shell npm view skia-canvas version)
.PHONY: optimized test debug visual check clean distclean release skia-version with-local-skia run preview
.DEFAULT_GOAL := $(LIB)

# platform-specific features to be passed to cargo
OS=$(shell sh -c 'uname -s 2>/dev/null')
ifeq ($(OS),Darwin)
	FEATURES = metal,window
else # Linux & Windows
	FEATURES = vulkan,window,freetype
endif

$(NPM):
	npm ci --ignore-scripts

$(LIB): $(NPM) $(LIB_SRC)
	@npm run build
	@touch $(LIB)

optimized: $(NPM)
	@rm -f $(LIB)
	@npm run build -- --release --features $(FEATURES)

test: $(LIB)
	@$(JEST) --verbose

debug: $(LIB)
	@$(JEST) --watch

visual: $(LIB)
	@$(NODEMON) test/visual -w native/index.node -w test/visual -e js,html

check:
	cargo check

clean:
	rm -rf $(LIBDIR)
	rm -rf $(CURDIR)/target/debug
	rm -rf $(CURDIR)/target/release

distclean: clean
	rm -rf $(NPM)
	rm -rf $(CURDIR)/build
	cargo clean

release:
	@if [[ `git status -s package.json` != "" ]]; then printf "Commit changes to package.json first:\n\n"; git --no-pager diff package.json; exit 1; fi
	@if [[ `git cherry -v` != "" ]]; then printf "Unpushed commits:\n\n"; git --no-pager log --branches --not --remotes; exit 1; fi
	@if [[ $(GIT_TAG) =~ ^v$(PACKAGE_VERSION) ]]; then printf "Already published $(GIT_TAG)\n"; exit 1; fi
	@echo
	@echo "Currently on NPM:  $(NPM_VERSION)"
	@echo "Package Version:   $(PACKAGE_VERSION)"
	@echo "Last Git Tag:     $(GIT_TAG)"
	@echo
	@/bin/echo -n "Update release -> v$(PACKAGE_VERSION)? [y/N] "
	@read line; if [[ $$line = "y" ]]; then printf "\nPushing tag to github..."; else exit 1; fi
	git tag -a v$(PACKAGE_VERSION) -m v$(PACKAGE_VERSION)
	git push origin --tags
	@printf "\nNext: publish the release on github to submit to npm\n"

# linux-build helpers
skia-version:
	@grep -m 1 '^skia-safe' Cargo.toml | egrep -o '[0-9\.]+'

with-local-skia:
	echo '' >> Cargo.toml
	echo '[patch.crates-io]' >> Cargo.toml
	echo 'skia-safe = { path = "../rust-skia/skia-safe" }' >> Cargo.toml
	echo 'skia-bindings = { path = "../rust-skia/skia-bindings" }' >> Cargo.toml

# debugging
run: $(LIB)
	@node check.js

preview: run
	@less out.png || true
