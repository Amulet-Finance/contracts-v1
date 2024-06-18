#!/bin/bash
DEFAULT_BRANCH="v15.0.0"
ORG="${ORG:-amulet}"
VERSION="${VERSION:-0.0.1}"

GAIA_TAG="gaia-${ORG}-${VERSION}"
BRANCH="${GAIA_VERSION:-$DEFAULT_BRANCH}"
DIR="$(dirname "$0")"
cd "$DIR" || exit 1
git clone --branch "$BRANCH" https://github.com/cosmos/gaia.git

if [ $? -ne 0 ]; then
    echo "Failed to clone repository."
    exit 1
fi

cp ./Dockerfile ./gaia

docker build -f ./gaia/Dockerfile -t "$GAIA_TAG" ./gaia

if [ $? -ne 0 ]; then
    echo "Failed to build Docker image."
    rm -rf ./gaia
    exit 1
fi

rm -rf ./gaia
echo "Successfully built $GAIA_TAG image."
