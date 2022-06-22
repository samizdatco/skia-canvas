FROM node:16-alpine

ENV RUSTFLAGS="-C target-feature=-crt-static" \
    CC="clang" \
    CXX="clang++" \
    GN_EXE="gn" \
    SKIA_GN_COMMAND="/usr/bin/gn" \
    SKIA_NINJA_COMMAND="/usr/bin/ninja"

RUN apk update && apk add --update --no-cache \
    bash curl git python3 perl clang llvm g++ build-base \
    musl-dev openssl-dev fontconfig-dev fontconfig ttf-dejavu

RUN apk add --update --no-cache --repository http://dl-cdn.alpinelinux.org/alpine/v3.15/community \
    python2

RUN apk add --update --no-cache --repository http://dl-cdn.alpinelinux.org/alpine/edge/testing \
    gn ninja

RUN export ASM="/usr/include/c++/10.3.1/aarch64-alpine-linux-musl/asm" && \
    mkdir -p ${ASM} && \
    touch ${ASM}/hwcap.h

WORKDIR /code
COPY alpine-build.patch .