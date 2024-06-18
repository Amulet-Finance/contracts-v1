#!/bin/sh
NODE=${NODE:-default-neutron_ics-1}
if [ "$LOGGER_LEVEL" = "trace" ]; then
  LOGGER_LEVEL="debug"
fi
echo "NODE is set to: ${NODE}"
echo "LOGGER_LEVEL is set to: ${LOGGER_LEVEL}"

echo "Waiting for the first block..."
while ! curl -f ${NODE}:1317/cosmos/base/tendermint/v1beta1/blocks/1 >/dev/null 2>&1; do
  sleep 1
done

echo "First block detected. Starting relayer..."
neutron_query_relayer start

# Additional debugging information
if [ $? -ne 0 ]; then
  echo "Relayer failed to start. Checking IBC connection status..."
  curl "${NODE}:1317/ibc/core/connection/v1/connections/connection-0"
fi
