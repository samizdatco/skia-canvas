FROM node:buster-slim

RUN apt-get update && \
    apt-get install -y -q \
    python2 python3 perl git clang lldb lld \
    build-essential software-properties-common \
    libssl-dev libfontconfig-dev ninja-build

RUN add-apt-repository "deb http://deb.debian.org/debian buster-backports main" && \
    apt-get update && apt-get install -t buster-backports -y -q \
    curl

RUN curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg && \
    echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | tee /etc/apt/sources.list.d/github-cli.list > /dev/null && \
    apt update && \
    apt install -y gh jq

ENV SKIA_NINJA_COMMAND="/usr/bin/ninja"