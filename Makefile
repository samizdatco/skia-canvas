REPO=https://github.com/google/skia/trunk/modules/canvaskit
UPSTREAM := $(CURDIR)/upstream
NEON := $(CURDIR)/node_modules/.bin/neon
JEST := $(CURDIR)/node_modules/.bin/jest
.PHONY: build run test clean upstream

all:
	@$(MAKE) build && $(MAKE) run

$(NEON):
	npm install

check:
	@cd native; cargo check

build: $(NEON)
	@$(NEON) build

run:
	@node scribble.js

test: $(NEON)
	@$(JEST)

clean:
	rm -rf native/dist

upstream:
	@mkdir -p $(UPSTREAM)/{bindings,htmlcanvas,module}
	@$(eval TMP := $(shell mktemp -d))
	svn checkout -q $(REPO) $(TMP)
	cp $(TMP)/canvaskit/LICENSE $(UPSTREAM)
	cp $(TMP)/*.cpp $(UPSTREAM)/bindings
	cp $(TMP)/*.js $(UPSTREAM)/module
	rm -f $(UPSTREAM)/module/karma*.js
	cp $(TMP)/htmlcanvas/*.js $(UPSTREAM)/htmlcanvas
	rm -rf $(TMP)