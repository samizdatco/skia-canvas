FROM node:16-alpine

ENV RUSTFLAGS="-C target-feature=-crt-static" \
    CC="clang" \
    CXX="clang++" \
    GN_EXE=gn \
    SKIA_GN_COMMAND="/usr/bin/gn" \
    SKIA_NINJA_COMMAND="/usr/bin/ninja"

RUN sed -i -e 's/v[[:digit:]]\..*\//edge\//g' /etc/apk/repositories
RUN apk add --update --no-cache --repository http://dl-cdn.alpinelinux.org/alpine/edge/testing \
    bash curl git tar python3 python2 perl clang llvm make gn ninja \
    musl-dev build-base fontconfig-dev openssl-dev g++ \
    fontconfig ttf-dejavu
RUN apk upgrade

WORKDIR /code
COPY alpine-build.patch .
