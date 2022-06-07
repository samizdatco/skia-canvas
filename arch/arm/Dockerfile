FROM arm32v7/debian:buster-slim

RUN apt-get update && \
    apt-get install -y \
      curl build-essential lsb-release wget software-properties-common \
      python2 libssl-dev libfontconfig-dev git clang lldb lld ninja-build

WORKDIR /usr/local/src
RUN git clone https://gn.googlesource.com/gn && \
    cd gn && \
    python build/gen.py && \
    ninja -C out && \
    cp out/gn /usr/local/bin/gn && \
    rm -rf /usr/local/src/gn

ENV SKIA_GN_COMMAND="/usr/local/bin/gn"
ENV SKIA_NINJA_COMMAND="/usr/bin/ninja"

RUN groupadd -r -g 1000 pi
RUN useradd -r -m -u 1000 -g pi pi
USER pi
