#!/usr/bin/env bash

# to create `aws-lambda-x64.zip`, run this from the project root:
#   docker run --platform linux/amd64 -v ./arch/lambda:/opt/run -v .:/mnt amazonlinux:2023 bash /opt/run/build-layer.sh
# to create `aws-lambda-arm64.zip`, run:
#   docker run --platform linux/arm64 -v ./arch/lambda:/opt/run -v .:/mnt amazonlinux:2023 bash /opt/run/build-layer.sh

yum -y install fontconfig nodejs zip git

case "$(uname -p)" in
 x86_64) LAYER_ZIP="aws-lambda-x64.zip" ;;
 aarch64) LAYER_ZIP="aws-lambda-arm64.zip" ;;
esac

LAYER_DIR=/opt/layer
PREBUILD=/opt/skia.node
REPO=https://github.com/samizdatco/skia-canvas

mkdir -p ${LAYER_DIR}/node && cd ${LAYER_DIR}/node
if [ -f $PREBUILD ]; then
    npm install --ignore-scripts $REPO
    cp $PREBUILD node_modules/skia-canvas/lib
else
    npm install $REPO
fi

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
