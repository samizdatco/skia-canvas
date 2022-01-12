FROM arm64v8/debian:buster-slim

RUN apt-get update && \
    apt-get install -y -q \
    curl build-essential lsb-release wget software-properties-common \
    python2 libssl-dev libfontconfig-dev git clang lldb lld ninja-build

ENV SKIA_NINJA_COMMAND="/usr/bin/ninja"

RUN groupadd -r -g 1000 pi
RUN useradd -r -u 1000 -g pi pi
USER pi
