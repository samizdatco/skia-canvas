FROM node:16-alpine3.14
ARG TARGETARCH

ENV RUSTFLAGS="-C target-feature=-crt-static" \
    CC="clang" \
    CXX="clang++" \
    GN_EXE="gn" \
    SKIA_GN_COMMAND="/usr/bin/gn" \
    SKIA_NINJA_COMMAND="/usr/bin/ninja"

# basics
RUN apk update && apk add --update --no-cache \
    bash curl git python3 perl build-base g++ linux-headers llvm clang \
    musl-dev openssl-dev fontconfig-dev fontconfig ttf-dejavu

# add ninja & gn for building skia
RUN apk add --update --no-cache --repository http://dl-cdn.alpinelinux.org/alpine/v3.16/community gn
RUN apk add --update --no-cache --repository http://dl-cdn.alpinelinux.org/alpine/edge/community ninja-build

# install gh
ARG GH_VERSION=2.55.0
ARG GH_URL=https://github.com/cli/cli/releases/download/v$GH_VERSION/gh_${GH_VERSION}_linux_$TARGETARCH.tar.gz
RUN curl -sL $GH_URL | tar xz --strip-components=2 -C /usr/local/bin

RUN adduser --uid 1001 --no-create-home --disabled-password --gecos "" github
