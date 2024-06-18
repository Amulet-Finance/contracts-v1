#!/bin/bash
# Function to find the project root directory
find_project_root() {
  local dir="$PWD"
  while [[ "$dir" != "/" ]]; do
    if [[ -f "$dir/package.json" ]]; then
      echo "$dir"
      return
    fi
    dir="$(dirname "$dir")"
  done
  echo "Project root not found" >&2
  exit 1
}

# Locate the project root
PROJECT_ROOT=$(find_project_root)

# Change to the project root directory
cd "$PROJECT_ROOT" || exit 1

VERSION=$(jq -r '.version' < package.json)
ORG="amulet"

export ORG
export VERSION

cd ./scripts/suite/dockerfiles

IMAGE_DIRS=$(ls -1 | grep -v build-all.sh | grep -v '^$')
for NAME in $IMAGE_DIRS; do
    # check if docker image is already built
    DOCKERIMAGE="$NAME-$ORG-$VERSION"

    if [[ "$(docker images -q $DOCKERIMAGE 2> /dev/null)" == "" ]]; then
        echo "Building $DOCKERIMAGE"

        ./$NAME/build.sh
    else
        echo "Image $DOCKERIMAGE already exists"
    fi

    echo ""
done

docker images
