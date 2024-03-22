/**
* This file was automatically generated by @cosmwasm/ts-codegen@0.35.7.
* DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
* and run the @cosmwasm/ts-codegen generate command to regenerate this file.
*/

export interface InstantiateMsg {}
export type ExecuteMsg = ExecuteMsg1 | ExecuteMsg2;
export type ExecuteMsg1 = {
  transfer_admin_role: {
    next_admin: string;
  };
} | {
  claim_admin_role: {};
} | {
  cancel_role_transfer: {};
};
export type ExecuteMsg2 = {
  create_synthetic: {
    decimals: number;
    ticker: string;
  };
} | {
  set_whitelisted: {
    minter: string;
    whitelisted: boolean;
  };
} | {
  mint: {
    amount: Uint128;
    recipient: string;
    synthetic: string;
  };
} | {
  burn: {};
};
export type Uint128 = string;
export type QueryMsg = QueryMsg1 | QueryMsg2;
export type QueryMsg1 = {
  current_admin: {};
} | {
  pending_admin: {};
};
export type QueryMsg2 = {
  whitelisted: {
    minter: string;
  };
} | {
  synthetic_metadata: {
    synthetic: string;
  };
};
export interface CurrentAdminResponse {
  current_admin?: string | null;
}
export interface PendingAdminResponse {
  pending_admin?: string | null;
}
export interface SyntheticMetadataResponse {
  decimals: number;
  ticker: string;
}
export interface WhitelistedResponse {
  whitelisted: boolean;
}