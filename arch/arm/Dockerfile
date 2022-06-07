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

RUN curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg && \
    echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | tee /etc/apt/sources.list.d/github-cli.list > /dev/null && \
    apt update && \
    apt install -y gh jq

ENV SKIA_GN_COMMAND="/usr/local/bin/gn"
ENV SKIA_NINJA_COMMAND="/usr/bin/ninja"

RUN groupadd -r -g 1000 pi
RUN useradd -r -m -u 1000 -g pi pi
USER pi
