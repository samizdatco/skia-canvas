#!/usr/bin/env bash

case "$(uname -p)" in
 x86_64) LAYER_ZIP="aws-lambda-x64.zip" ;;
 aarch64) LAYER_ZIP="aws-lambda-arm64.zip" ;;
esac

LAYER_DIR="/opt/layer"

yum -y install fontconfig nodejs zip git

mkdir -p ${LAYER_DIR}/node && \
    cd  ${LAYER_DIR}/node && \
    npm install https://github.com/samizdatco/skia-canvas

mkdir -p ${LAYER_DIR}/lib && \
    cd  ${LAYER_DIR}/lib && \
    cp /lib64/libfontconfig.so* . && \
    cp /lib64/libfreetype.so* . && \
    cp /lib64/libpng16.so* . && \
    cp /lib64/libharfbuzz.so* . && \
    cp /lib64/libgraphite2.so* . && \
    cp /lib64/libbrotlicommon.so* . && \
    cp /lib64/libbrotlidec.so* .

cd ${LAYER_DIR} && zip -r9 /mnt/${LAYER_ZIP} *
