NODEMON := $(CURDIR)/node_modules/.bin/nodemon
NEON := $(CURDIR)/node_modules/.bin/neon
JEST := $(CURDIR)/node_modules/.bin/jest
LIB := $(CURDIR)/native/index.node
SRC := $(shell find $(CURDIR)/native/src -regex ".*\.rs")
.PHONY: build run test check clean

all: run

$(NEON):
	npm install

$(LIB): $(NEON) $(SRC)
	@$(NEON) build

build: $(LIB)
	@echo build complete

run: $(LIB)
	@node scribble.js

preview: run
	@open -a Preview.app out.png
	@open -a "Visual Studio Code"

test: $(LIB)
	@$(JEST)

diff: $(LIB)
	@$(NODEMON) test/visual -w native/index.node -w test/visual -e js,html

check:
	@cd native; cargo check

clean:
	rm -rf native/target/debug
	rm -rf native/target/release
