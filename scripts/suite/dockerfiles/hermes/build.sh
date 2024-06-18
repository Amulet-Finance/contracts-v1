#! /usr/bin/env bash

ORG="${ORG:-amulet}"
VERSION="${VERSION:-v0.1.0}"
HERMES_TAG="hermes-$ORG-$VERSION"
DOCKER_HUB_IMAGE="informalsystems/hermes:v1.9.0"  # Specify the image and tag from Docker Hub

# Pull the image from Docker Hub
docker pull ${DOCKER_HUB_IMAGE}

if [ $? -ne 0 ]; then
    echo "Failed to pull Docker image from Docker Hub."
    exit 1
fi

# Tag the pulled image with your desired tag
docker tag ${DOCKER_HUB_IMAGE} ${HERMES_TAG}

if [ $? -ne 0 ]; then
    echo "Failed to tag Docker image."
    exit 1
fi

# Create the start-hermes.sh script
cat <<EOF > start-hermes.sh
#!/bin/bash
# Ensure the Hermes directory exists and set the correct permissions
mkdir -p /home/hermes/.hermes
chown -R hermes:hermes /home/hermes/.hermes

# Run hermes config auto
su - hermes -c "hermes config auto"

# Switch to the hermes user and run Hermes
su - hermes -c "/usr/bin/hermes \$@"
EOF

# Ensure the start-hermes.sh script has executable permissions
chmod +x start-hermes.sh

# Create a Dockerfile to add the start-hermes.sh script and update the entrypoint
cat <<EOF > Dockerfile.hermes
FROM ${HERMES_TAG}
USER root
COPY start-hermes.sh /root/start-hermes.sh
RUN chmod +x /root/start-hermes.sh
ENTRYPOINT ["/root/start-hermes.sh"]
EOF

# Build the new Docker image with the custom entrypoint
docker build -f Dockerfile.hermes -t ${HERMES_TAG} .

if [ $? -ne 0 ]; then
    echo "Failed to build Docker image with custom entrypoint."
    exit 1
fi

# Clean up
rm Dockerfile.hermes start-hermes.sh

echo "Successfully pulled, tagged, and updated ${HERMES_TAG} image."

