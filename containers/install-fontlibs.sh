#!/usr/bin/env bash
set -euxo pipefail

export CC=clang
export CXX=clang++

# download the first reachable URL from the list of mirrors
fetch_and_extract() {
  local out="/opt/$(basename "$1")" url
  for url in "$@"; do
    echo "fetching $url"
    if curl -fSL --retry 8 --retry-delay 5 --connect-timeout 30 -o "$out" "$url"; then
      tar xJf "$out" -C /opt
      return 0
    fi
    echo "  -> mirror failed, trying next" >&2
  done
  echo "all mirrors failed for $(basename "$1")" >&2
  return 1
}

# install an up-to-date version of meson
python3 -m venv /opt/venv
export PATH="/opt/venv/bin:$PATH"
pip install meson

# compile a dummy freetype lib (version-matched and configured to mirror the api surface of
# skia m150's embedded copy via the custom freetype.cfg)
FREETYPE=freetype-2.14.2
FREETYPE_URL=https://download.savannah.gnu.org/releases/freetype/${FREETYPE}.tar.xz
FREETYPE_URL_ALT=https://downloads.sourceforge.net/project/freetype/freetype2/${FREETYPE#freetype-}/${FREETYPE}.tar.xz
FREETYPE_CFG=/opt/freetype.cfg
fetch_and_extract "$FREETYPE_URL" "$FREETYPE_URL_ALT"
cd /opt/${FREETYPE} && \
   cp $FREETYPE_CFG modules.cfg && \
   ./configure --with-harfbuzz=no && \
   make && make install

# compile fontconfig (look for config in system dirs but install to /usr/local so we can extract the static lib)
FONTCONFIG_VERSION=2.18.3
FONTCONFIG=fontconfig-$FONTCONFIG_VERSION
FONTCONFIG_URL=https://gitlab.freedesktop.org/api/v4/projects/890/packages/generic/fontconfig/$FONTCONFIG_VERSION/${FONTCONFIG}.tar.xz
fetch_and_extract "$FONTCONFIG_URL"
cd /opt/${FONTCONFIG} && \
    meson setup -Dprefix=/ -Dsysconfdir=/etc -Dlocalstatedir=/var -Ddefault_library=static -Dprefer_static=true -Dxml-backend=expat -Dtests=disabled build && \
    meson compile -C build && \
    meson install --destdir=/usr/local -C build
