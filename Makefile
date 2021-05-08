NAPI_VERSION := 6
NPM := $(CURDIR)/node_modules
NODEMON := $(CURDIR)/node_modules/.bin/nodemon
JEST := $(CURDIR)/node_modules/.bin/jest
LIBDIR := $(CURDIR)/lib/v$(NAPI_VERSION)
LIB := $(LIBDIR)/index.node
.PHONY: build run test check clean visual package publish

$(NPM):
	npm install

$(LIB): $(NPM)
	npm run build

build: $(LIB)
	@npm run build
	@echo build complete

test: $(LIB)
	@$(JEST)

visual: $(LIB)
	@$(NODEMON) test/visual -w native/index.node -w test/visual -e js,html

check:
	cargo check

clean:
	@rm $(LIB)
	@rmdir $(LIBDIR)

distclean:
	cargo clean

run: $(LIB)
	@npm run build
	@node check.js

preview: run
	@open -a Preview.app out.png
	@open -a "Visual Studio Code"