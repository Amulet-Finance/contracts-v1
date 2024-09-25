/**
* This file was automatically generated by @cosmwasm/ts-codegen@1.11.1.
* DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
* and run the @cosmwasm/ts-codegen generate command to regenerate this file.
*/

export interface InstantiateMsg {
  hub_address: string;
}
export type ExecuteMsg = ExecuteMsg1 | ProxyExecuteMsg;
export type ExecuteMsg1 = {
  transfer_admin_role: {
    next_admin: string;
  };
} | {
  claim_admin_role: {};
} | {
  cancel_role_transfer: {};
};
export type ProxyExecuteMsg = {
  set_whitelisted: {
    address: string;
    whitelisted: boolean;
  };
} | {
  deposit: {
    vault: string;
  };
} | {
  mint: {
    vault: string;
  };
} | {
  advance: {
    amount: Uint128;
    vault: string;
  };
} | {
  redeem: {
    vault: string;
  };
};
export type Uint128 = string;
export type QueryMsg = QueryMsg1 | ProxyQueryMsg;
export type QueryMsg1 = {
  current_admin: {};
} | {
  pending_admin: {};
};
export type ProxyQueryMsg = {
  config: {};
} | {
  whitelisted: {
    address: string;
  };
};
export interface ConfigResponse {
  hub_address: string;
}
export interface CurrentAdminResponse {
  current_admin?: string | null;
}
export interface PendingAdminResponse {
  pending_admin?: string | null;
}
export interface WhitelistedResponse {
  whitelisted: boolean;
}