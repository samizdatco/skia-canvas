#!/usr/bin/env bash
# see `skia-canvas-lambda.zip` recipe in Makefile for invocation details

export NODE_DIR="/opt/layer/nodejs"
export LAYER_DIR="/opt/layer"
export LAYER_ZIP="/mnt/aws-lambda-x86.zip"

yum -y install fontconfig nodejs zip git

mkdir -p ${NODE_DIR} && \
    cd  ${NODE_DIR} && \
    npm install https://github.com/samizdatco/skia-canvas

mkdir -p ${LAYER_DIR}/lib && \
    cp /lib64/libfontconfig.so* ${LAYER_DIR}/lib && \
    cp /lib64/libfreetype.so* ${LAYER_DIR}/lib && \
    cp /lib64/libpng16.so* ${LAYER_DIR}/lib && \
    cp /lib64/libharfbuzz.so* ${LAYER_DIR}/lib && \
    cp /lib64/libgraphite2.so* ${LAYER_DIR}/lib && \
    cp /lib64/libbrotlicommon.so* ${LAYER_DIR}/lib && \
    cp /lib64/libbrotlidec.so* ${LAYER_DIR}/lib

cd ${LAYER_DIR} && zip -r9 ${LAYER_ZIP} *
