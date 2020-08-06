NEON := $(CURDIR)/node_modules/.bin/neon
JEST := $(CURDIR)/node_modules/.bin/jest
LIB := $(CURDIR)/native/index.node
SRC := $(wildcard $(CURDIR)/native/src/*)
.PHONY: build run test check clean

all: run

$(NEON):
	npm install

$(LIB): $(NEON) $(SRC)
	@$(NEON) build

build: $(LIB)

run: $(LIB)
	@node scribble.js

test: $(LIB)
	@$(JEST)

check:
	@cd native; cargo check

clean:
	rm -rf native/target/debug
	rm -rf native/target/release
