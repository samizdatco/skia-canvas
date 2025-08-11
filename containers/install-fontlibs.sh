#!/usr/bin/env bash
set -euxo pipefail

export CC=clang
export CXX=clang++

# install an up-to-date version of meson
python3 -m venv /opt/venv
export PATH="/opt/venv/bin:$PATH"
pip install meson

# compile dummy freetype lib (meant to mirror the api surface of skia's embedded copy via the custom modules.cfg)
FREETYPE=freetype-2.13.3
FREETYPE_URL=https://download.savannah.gnu.org/releases/freetype/${FREETYPE}.tar.xz
FREETYPE_CFG=/opt/freetype.cfg
curl -sL $FREETYPE_URL | tar xJf - -C /opt
cd /opt/${FREETYPE} && \
   cp $FREETYPE_CFG modules.cfg && \
   make && make install

# compile fontconfig (look for config in system dirs but install to /usr/local so we can extract the static lib)
FONTCONFIG_VERSION=2.17.1
FONTCONFIG=fontconfig-$FONTCONFIG_VERSION
FONTCONFIG_URL=https://gitlab.freedesktop.org/api/v4/projects/890/packages/generic/fontconfig/$FONTCONFIG_VERSION/${FONTCONFIG}.tar.xz
curl -sL $FONTCONFIG_URL | tar xJf - -C /opt
cd /opt/${FONTCONFIG} && \
    meson setup -Dprefix=/ -Dsysconfdir=/etc -Dlocalstatedir=/var -Ddefault_library=static -Dprefer_static=true -Dxml-backend=expat -Dtests=disabled build && \
    meson compile -C build && \
    meson install --destdir=/usr/local -C build
