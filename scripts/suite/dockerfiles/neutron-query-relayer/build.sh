#! /usr/bin/env bash

ORG="${ORG:-amulet}"
VERSION="${VERSION:-0.0.1}"
NEUTRON_RELAYER_TAG="neutron-relayer-$ORG-$VERSION"

DIR="$(dirname $0)"
cd $DIR

git clone -b foxpy/low-submission-margin-period https://github.com/neutron-org/neutron-query-relayer

if [ $? -ne 0 ]; then
    echo "Failed to clone repository."
    exit 1
fi

cp ./run.sh ./neutron-query-relayer/run.sh
cp ./Dockerfile ./neutron-query-relayer/Dockerfile

cd neutron-query-relayer

docker build . -t ${NEUTRON_RELAYER_TAG} --progress=plain --no-cache --platform=linux/amd64

if [ $? -ne 0 ]; then
    echo "Failed to build Docker image."
    cd ..
    rm -rf ./neutron-query-relayer
    exit 1
fi

cd ..
rm -rf ./neutron-query-relayer

echo "Successfully built $NEUTRON_RELAYER_TAG image."

