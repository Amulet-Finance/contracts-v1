#!/bin/bash
set -e

BINARY=${BINARY:-neutrond}
BASE_DIR=./data
CHAINID=${CHAINID:-test-1}
STAKEDENOM=${STAKEDENOM:-untrn}
FEEMARKET_ENABLED=${FEEMARKET_ENABLED:-true}

# IMPORTANT! minimum_gas_prices should always contain at least one record, otherwise the chain will not start or halt
MIN_GAS_PRICES_DEFAULT='[{"denom":"ibc/27394FB092D2ECCD56123C74F36E4C1F926001CEADA9CA97EA622B25F41E5EB2","amount":"0"},{"denom":"untrn","amount":"0"}]'
MIN_GAS_PRICES=${MIN_GAS_PRICES:-"$MIN_GAS_PRICES_DEFAULT"}

BYPASS_MIN_FEE_MSG_TYPES_DEFAULT='["/ibc.core.channel.v1.Msg/RecvPacket", "/ibc.core.channel.v1.Msg/Acknowledgement", "/ibc.core.client.v1.Msg/UpdateClient"]'
BYPASS_MIN_FEE_MSG_TYPES=${BYPASS_MIN_FEE_MSG_TYPES:-"$BYPASS_MIN_FEE_MSG_TYPES_DEFAULT"}

MAX_TOTAL_BYPASS_MIN_FEE_MSG_GAS_USAGE_DEFAULT=1000000
MAX_TOTAL_BYPASS_MIN_FEE_MSG_GAS_USAGE=${MAX_TOTAL_BYPASS_MIN_FEE_MSG_GAS_USAGE:-"$MAX_TOTAL_BYPASS_MIN_FEE_MSG_GAS_USAGE_DEFAULT"}

CHAIN_DIR="$BASE_DIR/$CHAINID"
GENESIS_PATH="$CHAIN_DIR/config/genesis.json"

ADMIN_ADDRESS=$($BINARY keys show demowallet1 -a --home "$CHAIN_DIR" --keyring-backend test)

echo "Add consumer section..."
$BINARY add-consumer-section --home "$CHAIN_DIR"

### PARAMETERS SECTION

## slashing params
SLASHING_SIGNED_BLOCKS_WINDOW=140000
SLASHING_MIN_SIGNED=0.050000000000000000
SLASHING_FRACTION_DOUBLE_SIGN=0.010000000000000000
SLASHING_FRACTION_DOWNTIME=0.000100000000000000

function check_json() {
  MSG=$1
  if ! jq -e . >/dev/null 2>&1 <<<"$MSG"; then
      echo "Failed to parse JSON for $MSG" >&2
      exit 1
  fi
}

function set_genesis_param() {
  param_name=$1
  param_value=$2
  sed -i -e "s;\"$param_name\":.*;\"$param_name\": $param_value;g" "$GENESIS_PATH"
}

function set_genesis_param_jq() {
  param_path=$1
  param_value=$2
  jq "${param_path} = ${param_value}" > tmp_genesis_file.json < "$GENESIS_PATH" && mv tmp_genesis_file.json "$GENESIS_PATH"
}

function convert_bech32_base64_esc() {
  $BINARY keys parse $1 --output json | jq .bytes | xxd -r -p | base64 | sed -e 's/\//\\\//g'
}

set_genesis_param admins                                 "[\"$ADMIN_ADDRESS\"]"                      # admin module
set_genesis_param treasury_address                       "\"$ADMIN_ADDRESS\""                        # feeburner
set_genesis_param fee_collector_address                  "\"$ADMIN_ADDRESS\""                        # tokenfactory
set_genesis_param signed_blocks_window                   "\"$SLASHING_SIGNED_BLOCKS_WINDOW\","       # slashing
set_genesis_param min_signed_per_window                  "\"$SLASHING_MIN_SIGNED\","                 # slashing
set_genesis_param slash_fraction_double_sign             "\"$SLASHING_FRACTION_DOUBLE_SIGN\","       # slashing
set_genesis_param slash_fraction_downtime                "\"$SLASHING_FRACTION_DOWNTIME\""           # slashing
set_genesis_param minimum_gas_prices                     "$MIN_GAS_PRICES,"                          # globalfee
set_genesis_param max_total_bypass_min_fee_msg_gas_usage "\"$MAX_TOTAL_BYPASS_MIN_FEE_MSG_GAS_USAGE\"" # globalfee
set_genesis_param_jq ".app_state.globalfee.params.bypass_min_fee_msg_types" "$BYPASS_MIN_FEE_MSG_TYPES" # globalfee
set_genesis_param proposer_fee                           "\"0.25\""                                  # builder(POB)
set_genesis_param sudo_call_gas_limit                    "\"1000000\""                               # contractmanager
set_genesis_param max_gas                                "\"1000000000\""                            # consensus_params
set_genesis_param vote_extensions_enable_height          "\"1\""                                     # consensus_params
set_genesis_param_jq ".app_state.marketmap.params.admin" "\"$ADMIN_ADDRESS\""                        # marketmap
set_genesis_param_jq ".app_state.marketmap.params.market_authorities" "[\"$ADMIN_ADDRESS\"]"         # marketmap
set_genesis_param_jq ".app_state.feemarket.params.min_base_gas_price" "\"0.0025\""                   # feemarket
set_genesis_param_jq ".app_state.feemarket.params.fee_denom" "\"untrn\""                             # feemarket
set_genesis_param_jq ".app_state.feemarket.params.max_learning_rate" "\"0.5\""                       # feemarket
set_genesis_param_jq ".app_state.feemarket.params.enabled" "$FEEMARKET_ENABLED"                      # feemarket
set_genesis_param_jq ".app_state.feemarket.params.distribute_fees" "true"                            # feemarket
set_genesis_param_jq ".app_state.feemarket.state.base_gas_price" "\"0.0025\""                        # feemarket

if ! jq -e . "$GENESIS_PATH" >/dev/null 2>&1; then
    echo "genesis appears to become incorrect json" >&2
    exit 1
fi

echo "Genesis parameters set successfully"

