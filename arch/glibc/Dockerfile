FROM node:buster-slim
ARG TARGETARCH

ENV PATH="/usr/local/bin:$PATH" \
    SKIA_NINJA_COMMAND="/usr/bin/ninja"

# basics
RUN apt-get update && \
    apt-get install -y -q \
        perl build-essential software-properties-common \
        libssl-dev libfontconfig-dev

# upgrade curl and add deps for building git & skia
RUN add-apt-repository "deb http://archive.debian.org/debian buster-backports main" && \
    apt-get update && apt-get install -y -q -t buster-backports\
        curl wget libghc-zlib-dev libcurl4-gnutls-dev gettext ninja-build

# upgrade git to 2.47
ARG GIT_VERSION=2.47.0
ARG GIT_URL=https://github.com/git/git/archive/refs/tags/v$GIT_VERSION.tar.gz
RUN curl -sL $GIT_URL | tar xz -C /opt && \
    cd /opt/git-$GIT_VERSION && \
    make -j 12 && make prefix=/usr/local install

# downgrade to the libcurl version that doesn't make git throw HTTP2 errors:
# https://bugs.debian.org/cgi-bin/bugreport.cgi?bug=987187
RUN apt-get reinstall -y -q -t buster --allow-downgrades libcurl3-gnutls=7.64.0-4+deb10u2 && \
    apt-mark hold libcurl3-gnutls

# upgrade clang to v18
ARG LLVM_VERSION=18
COPY install-llvm.sh .
RUN bash install-llvm.sh $LLVM_VERSION

# install gh
ARG GH_VERSION=2.55.0
ARG GH_URL=https://github.com/cli/cli/releases/download/v$GH_VERSION/gh_${GH_VERSION}_linux_$TARGETARCH.tar.gz
RUN curl -sL $GH_URL | tar xz --strip-components=2 -C /usr/local/bin

RUN useradd -u 1001 -M github
