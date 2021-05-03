NODEMON := $(CURDIR)/node_modules/.bin/nodemon
NEON := $(CURDIR)/node_modules/.bin/neon
JEST := $(CURDIR)/node_modules/.bin/jest
LIB := $(CURDIR)/native/index.node
SRC := $(shell find $(CURDIR)/native/src -regex ".*\.rs")
GIT_TAG = $(shell git describe)
PACKAGE_VERSION = $(shell npm run env | grep npm_package_version | cut -d '=' -f 2)
RUST_SKIA_VERSION = $(shell egrep 'RUST_SKIA_TAG\s*=' .travis.yml | sed 's/[- ]*RUST_SKIA_TAG=\"\(.*\)\"/\1/')
NPM_VERSION = $(shell npm view skia-canvas version)
.PHONY: build run test check clean visual package publish


all: build

$(NEON):
	npm install

$(LIB): $(NEON) $(SRC)
	@$(NEON) build

build: $(LIB)
	@echo build complete

test: $(LIB)
	@$(JEST)

visual: $(LIB)
	@$(NODEMON) test/visual -w native/index.node -w test/visual -e js,html

check:
	@cd native; cargo check

clean:
	rm -rf native/target/debug
	rm -rf native/target/release

package:
	@if [[ `git status -s package.json` != "" ]]; then echo "Commit changes to package.json first:"; git --no-pager diff package.json; exit 1; fi
	@if [[ `git cherry -v` != "" ]]; then echo "Unpushed commits:"; git --no-pager log --branches --not --remotes; exit 1; fi
	@if [[ $(GIT_TAG) =~ ^v$(PACKAGE_VERSION) ]]; then echo "Already published $(GIT_TAG)"; exit 1; fi
	@echo "NPM Version: $(NPM_VERSION)"
	@echo "Skia Canvas: $(PACKAGE_VERSION)"
	@echo "   Bindings: $(RUST_SKIA_VERSION)"
	@echo
	@/bin/echo -n "Update release -> v$(PACKAGE_VERSION)? [y/N] "
	@read line; if [[ $$line = "y" ]]; then echo "Pushing tag to github..."; else exit 1; fi
	git tag -a v$(PACKAGE_VERSION) -m v$(PACKAGE_VERSION)
	git push origin --tags
	@echo "Next: run 'make publish' when travis’s build completes..."

publish:
	@echo "NPM Version: $(NPM_VERSION)"
	@echo "Skia Canvas: $(PACKAGE_VERSION)"
	@echo "   Bindings: $(RUST_SKIA_VERSION)"
	@echo
	@if [[ $(GIT_TAG) != v$(PACKAGE_VERSION) ]]; then echo "Modifications since tag $(GIT_TAG)"; exit 1; fi
	@if [[ $(NPM_VERSION) = $(PACKAGE_VERSION) ]]; then echo "Already published $(PACKAGE_VERSION)"; exit 1; fi
	npm publish --dry-run
	@echo
	@/bin/echo -n "Update NPM package -> v$(PACKAGE_VERSION)? [y/N] "
	@read line; if [[ $$line = "y" ]]; then echo "Publishing to NPM..."; else exit 1; fi
	npm publish
	@echo "Next: publish the draft release at https://github.com/samizdatco/skia-canvas/releases"

