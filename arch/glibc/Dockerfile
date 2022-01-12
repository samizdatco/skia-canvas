FROM node:lts-stretch-slim

RUN apt-get update && apt-get install -y -q software-properties-common
RUN add-apt-repository "deb http://deb.debian.org/debian stretch-backports main"
RUN apt-get update && apt-get install -y \
    curl less build-essential lsb-release wget python2.7  \
    libssl-dev libfontconfig-dev git clang-5.0 lld-5.0

RUN ln -s /usr/bin/clang++-5.0   /usr/bin/clang++
RUN ln -s /usr/bin/clang-5.0     /usr/bin/clang
RUN ln -s /usr/bin/clang-cpp-5.0 /usr/bin/clang-cpp
RUN ln -s /usr/bin/ld.lld-5.0    /usr/bin/ld.lld
RUN ln -s /usr/bin/lld-5.0       /usr/bin/lld

RUN groupadd -r -g 1001 ghuser
RUN useradd -r -u 1001 -g ghuser ghuser
USER ghuser
