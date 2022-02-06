NAPI_VERSION := 6
NPM := $(CURDIR)/node_modules
NODEMON := $(CURDIR)/node_modules/.bin/nodemon
JEST := $(CURDIR)/node_modules/.bin/jest
LIBDIR := $(CURDIR)/lib/v$(NAPI_VERSION)
LIB := $(LIBDIR)/index.node
GIT_TAG = $(shell git describe)
PACKAGE_VERSION = $(shell npm run env | grep npm_package_version | cut -d '=' -f 2)
NPM_VERSION = $(shell npm view skia-canvas version)
.PHONY: build test visual check clean distclean release run preview

build: $(NPM)
	@rm -f $(LIB)
	@npm run build

$(NPM):
	npm install

$(LIB): build

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

run: build
	@node check.js

preview: run
	@open -a Preview.app out.png
	@open -a "Visual Studio Code"
