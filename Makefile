NPM := $(CURDIR)/node_modules
NODEMON := $(CURDIR)/node_modules/.bin/nodemon
JEST := $(CURDIR)/node_modules/.bin/jest
LIB := $(CURDIR)/lib/skia.node
LIB_SRC := Cargo.toml lib/prebuild.mjs $(wildcard src/*.rs) $(wildcard src/*/*.rs) $(wildcard src/*/*/*.rs)
GIT_TAG = $(shell git describe)
PACKAGE_VERSION = $(shell npm run env | grep npm_package_version | sed -e 's/^.*=/v/')
PRERELEASE_FLAG = $(subst -rc,--prerelease,$(findstring -rc,$(PACKAGE_VERSION)))
NPM_VERSION = $(shell npm view skia-canvas version)
.PHONY: optimized dev test debug visual check clean distclean release skia-version with-local-skia
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
	@$(JEST) --verbose

debug: $(LIB)
	@$(JEST) --watch

visual: $(LIB)
	@$(NODEMON) test/visual -w native/index.node -w test/visual -e js,html

check:
	cargo check

clean:
	rm -f $(LIB)

distclean: clean
	rm -rf $(NPM)
	rm -rf $(CURDIR)/target/debug
	rm -rf $(CURDIR)/target/release
	cargo clean

release:
	@if [[ `git status -s package.json` != "" ]]; then printf "Commit changes to package.json first:\n\n"; git --no-pager diff package.json; exit 1; fi
	@if [[ `git cherry -v` != "" ]]; then printf "Unpushed commits:\n"; git --no-pager log --oneline main --not --remotes="*/main"; exit 1; fi
	@if gh release view $(PACKAGE_VERSION) --json id > /dev/null; then printf "Already published $(PACKAGE_VERSION)\n"; exit 1; fi
	@echo
	@echo "Currently on NPM:  $(NPM_VERSION)"
	@echo "Last Git Tag:     $(GIT_TAG)"
	@echo "Package Version:  $(PACKAGE_VERSION)"
	@echo
	@/bin/echo -n "Update release -> $(PACKAGE_VERSION)? [y/N] "
	@read line; if [[ $$line = "y" ]]; then printf "\nPushing tag to github...\n"; else exit 1; fi
	@git tag -a $(PACKAGE_VERSION) -m $(PACKAGE_VERSION)
	@git push origin --tags
	@printf "\nCreating new release...\n"
	@gh release create $(PACKAGE_VERSION) $(PRERELEASE_FLAG) --draft --fail-on-no-commits --generate-notes
	@printf "\nNext: publish the release on github to submit to npm\n"

# linux-build helpers
skia-version:
	@grep -m 1 '^skia-safe' Cargo.toml | egrep -o '[0-9\.]+'

with-local-skia:
	echo '' >> Cargo.toml
	echo '[patch.crates-io]' >> Cargo.toml
	echo 'skia-safe = { path = "../rust-skia/skia-safe" }' >> Cargo.toml
	echo 'skia-bindings = { path = "../rust-skia/skia-bindings" }' >> Cargo.toml
