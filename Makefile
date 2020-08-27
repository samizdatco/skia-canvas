NODEMON := $(CURDIR)/node_modules/.bin/nodemon
NEON := $(CURDIR)/node_modules/.bin/neon
JEST := $(CURDIR)/node_modules/.bin/jest
LIB := $(CURDIR)/native/index.node
SRC := $(shell find $(CURDIR)/native/src -regex ".*\.rs")
GIT_TAG = $(shell git describe)
PACKAGE_VERSION := $(shell npm run env | grep npm_package_version | cut -d '=' -f 2)
RUST_SKIA_VERSION := $(shell egrep 'RUST_SKIA_TAG\s*=' .travis.yml | sed 's/[- ]*RUST_SKIA_TAG=\"\(.*\)\"/\1/')
.PHONY: build run test check clean visual publish

# git push --delete origin v0.9.16
# git tag -d v0.9.16


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
	@if [ $(GIT_TAG) = "v$(PACKAGE_VERSION)" ]; then echo "Already published $(GIT_TAG)"; exit 1; fi
	@echo "Current release: $(GIT_TAG)"
	@echo Skia Canvas: $(PACKAGE_VERSION)
	@echo Rust-Skia: $(RUST_SKIA_VERSION)
	@/bin/echo -n "Update release -> v$(PACKAGE_VERSION)? [y/N] "
	@read line; if [[ $$line = "y" ]]; then echo "Pushing tag to github..."; else exit 1; fi
	git tag -a v$(PACKAGE_VERSION) -m v$(PACKAGE_VERSION)
	git push origin --tags
	@echo "Call 'npm publish' when travis’s build completes..."
