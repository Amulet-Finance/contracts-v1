#! /usr/bin/env bash

ORG="${ORG:-amulet}"
VERSION="${VERSION:-v0.1.0}"
NEUTRON_TAG="neutron-$ORG-$VERSION"

DIR="$(dirname $0)"
cd $DIR

# Clone the repository
git clone https://github.com/neutron-org/neutron

if [ $? -ne 0 ]; then
    echo "Failed to clone repository."
    exit 1
fi

cd neutron

# Checkout the specific commit or branch
COMMIT_HASH_OR_BRANCH="v3.0.6"
git checkout $COMMIT_HASH_OR_BRANCH

# Modify the Dockerfile
sed -i '/^CMD bash \/opt\/neutron\/network\/init.sh && \\/d' Dockerfile
sed -i '/^    bash \/opt\/neutron\/network\/init-neutrond.sh && \\/d' Dockerfile
sed -i '/^    bash \/opt\/neutron\/network\/start.sh$/d' Dockerfile
echo 'ENTRYPOINT ["neutrond"]' >> Dockerfile

# Build the Docker image
docker buildx build --load --build-context app=. -t ${NEUTRON_TAG} --build-arg BINARY=neutrond .

if [ $? -ne 0 ]; then
    echo "Failed to build Docker image."
    exit 1
fi

cd ..
rm -rf ./neutron

echo "Successfully built $NEUTRON_TAG image."

