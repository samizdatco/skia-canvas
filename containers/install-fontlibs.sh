#!/usr/bin/env bash
set -euxo pipefail

export CC=clang
export CXX=clang++

# install an up-to-date version of meson
python3 -m venv /opt/venv
export PATH="/opt/venv/bin:$PATH"
pip install meson

# compile harfbuzz (prerequisite for building freetype below)
HARFBUZZ_VERSION=11.3.3
HARFBUZZ=harfbuzz-$HARFBUZZ_VERSION
HARFBUZZ_URL=https://github.com/harfbuzz/harfbuzz/releases/download/$HARFBUZZ_VERSION/${HARFBUZZ}.tar.xz
curl -sL $HARFBUZZ_URL | tar xJf - -C /opt
cd /opt/${HARFBUZZ} && \
  meson setup -Ddefault_library=static -Dprefer_static=true -Dfreetype=enabled -Dtests=disabled build && \
  meson compile -C build && \
  meson install -C build

# compile fontconfig (look for config in system dirs but install to /usr/local so we can extract the static lib)
FONTCONFIG_VERSION=2.17.1
FONTCONFIG=fontconfig-$FONTCONFIG_VERSION
FONTCONFIG_URL=https://gitlab.freedesktop.org/api/v4/projects/890/packages/generic/fontconfig/$FONTCONFIG_VERSION/${FONTCONFIG}.tar.xz
curl -sL $FONTCONFIG_URL | tar xJf - -C /opt
cd /opt/${FONTCONFIG} && \
    meson setup -Dprefix=/ -Dsysconfdir=/etc -Dlocalstatedir=/var -Ddefault_library=static -Dprefer_static=true -Dxml-backend=expat -Dtests=disabled build && \
    meson compile -C build && \
    meson install --destdir=/usr/local -C build

# compile freetype (disable bzip2 and inline gz support so we don't need to add additional libs in binaries_config.rs)
FREETYPE=freetype-2.13.3
FREETYPE_URL=https://download.savannah.gnu.org/releases/freetype/${FREETYPE}.tar.xz
curl -sL $FREETYPE_URL | tar xJf - -C /opt
cd /opt/${FREETYPE} && \
    meson setup -Ddefault_library=static -Ddefault_both_libraries=static -Dprefer_static=true -Dzlib=internal -Dbrotli=enabled -Dbzip2=disabled -Dharfbuzz=enabled -Dbuildtype=release build && \
    meson compile -C build && \
    meson install -C build
