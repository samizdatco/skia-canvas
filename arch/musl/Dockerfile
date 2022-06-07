FROM node:16-alpine

ENV RUSTFLAGS="-C target-feature=-crt-static" \
    CC="clang" \
    CXX="clang++" \
    GN_EXE="gn" \
    SKIA_GN_COMMAND="/usr/bin/gn" \
    SKIA_NINJA_COMMAND="/usr/bin/ninja"

RUN apk update && apk add --update --no-cache \
    bash curl jq git python3 python2 perl clang llvm g++ build-base \
    musl-dev openssl-dev fontconfig-dev fontconfig ttf-dejavu

RUN apk add --update --no-cache --repository http://dl-cdn.alpinelinux.org/alpine/edge/testing \
    gn ninja

RUN echo "@community http://dl-cdn.alpinelinux.org/alpine/edge/community" >> /etc/apk/repositories && \
    apk add github-cli@community

RUN export ASM="/usr/include/c++/10.3.1/aarch64-alpine-linux-musl/asm" && \
    mkdir -p ${ASM} && \
    touch ${ASM}/hwcap.h

WORKDIR /code
COPY alpine-build.patch .