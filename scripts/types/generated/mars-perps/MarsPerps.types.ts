// @ts-nocheck
/**
 * This file was automatically generated by @cosmwasm/ts-codegen@1.10.0.
 * DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
 * and run the @cosmwasm/ts-codegen generate command to regenerate this file.
 */

export type OracleBaseForString = string
export type ParamsBaseForString = string
export type Decimal = string
export interface InstantiateMsg {
  address_provider: string
  base_denom: string
  cooldown_period: number
  credit_manager: string
  deleverage_enabled: boolean
  max_positions: number
  oracle: OracleBaseForString
  params: ParamsBaseForString
  protocol_fee_rate: Decimal
  target_vault_collateralization_ratio: Decimal
  vault_withdraw_enabled: boolean
}
export type ExecuteMsg =
  | {
      update_owner: OwnerUpdate
    }
  | {
      deposit: {
        account_id?: string | null
      }
    }
  | {
      unlock: {
        account_id?: string | null
        shares: Uint128
      }
    }
  | {
      withdraw: {
        account_id?: string | null
      }
    }
  | {
      execute_perp_order: {
        account_id: string
        denom: string
        reduce_only?: boolean | null
        size: SignedUint
      }
    }
  | {
      close_all_positions: {
        account_id: string
        action?: ActionKind | null
      }
    }
  | {
      deleverage: {
        account_id: string
        denom: string
      }
    }
  | {
      update_params: {
        params: PerpParams
      }
    }
  | {
      update_config: {
        updates: ConfigUpdates
      }
    }
export type OwnerUpdate =
  | {
      propose_new_owner: {
        proposed: string
      }
    }
  | 'clear_proposed'
  | 'accept_proposed'
  | 'abolish_owner_role'
  | {
      set_emergency_owner: {
        emergency_owner: string
      }
    }
  | 'clear_emergency_owner'
export type Uint128 = string
export type ActionKind = 'default' | 'liquidation'
export interface SignedUint {
  abs: Uint128
  negative: boolean
  [k: string]: unknown
}
export interface PerpParams {
  closing_fee_rate: Decimal
  denom: string
  enabled: boolean
  liquidation_threshold: Decimal
  max_funding_velocity: Decimal
  max_loan_to_value: Decimal
  max_long_oi_value: Uint128
  max_net_oi_value: Uint128
  max_position_value?: Uint128 | null
  max_short_oi_value: Uint128
  min_position_value: Uint128
  opening_fee_rate: Decimal
  skew_scale: Uint128
}
export interface ConfigUpdates {
  address_provider?: string | null
  cooldown_period?: number | null
  credit_manager?: string | null
  deleverage_enabled?: boolean | null
  max_positions?: number | null
  oracle?: OracleBaseForString | null
  params?: ParamsBaseForString | null
  protocol_fee_rate?: Decimal | null
  target_vault_collateralization_ratio?: Decimal | null
  vault_withdraw_enabled?: boolean | null
}
export type QueryMsg =
  | {
      owner: {}
    }
  | {
      config: {}
    }
  | {
      vault: {
        action?: ActionKind | null
      }
    }
  | {
      denom_state: {
        denom: string
      }
    }
  | {
      perp_denom_state: {
        action: ActionKind
        denom: string
      }
    }
  | {
      perp_denom_states: {
        action: ActionKind
        limit?: number | null
        start_after?: string | null
      }
    }
  | {
      denom_states: {
        limit?: number | null
        start_after?: string | null
      }
    }
  | {
      perp_vault_position: {
        account_id?: string | null
        action?: ActionKind | null
        user_address: string
      }
    }
  | {
      deposit: {
        account_id?: string | null
        user_address: string
      }
    }
  | {
      unlocks: {
        account_id?: string | null
        user_address: string
      }
    }
  | {
      position: {
        account_id: string
        denom: string
        order_size?: SignedUint | null
      }
    }
  | {
      positions: {
        limit?: number | null
        start_after?: [string, string] | null
      }
    }
  | {
      positions_by_account: {
        account_id: string
        action?: ActionKind | null
      }
    }
  | {
      total_pnl: {}
    }
  | {
      opening_fee: {
        denom: string
        size: SignedUint
      }
    }
  | {
      denom_accounting: {
        denom: string
      }
    }
  | {
      total_accounting: {}
    }
  | {
      denom_realized_pnl_for_account: {
        account_id: string
        denom: string
      }
    }
  | {
      position_fees: {
        account_id: string
        denom: string
        new_size: SignedUint
      }
    }
export interface ConfigForString {
  address_provider: string
  base_denom: string
  cooldown_period: number
  credit_manager: string
  deleverage_enabled: boolean
  max_positions: number
  oracle: OracleBaseForString
  params: ParamsBaseForString
  protocol_fee_rate: Decimal
  target_vault_collateralization_ratio: Decimal
  vault_withdraw_enabled: boolean
}
export interface Accounting {
  balance: Balance
  cash_flow: CashFlow
  withdrawal_balance: Balance
}
export interface Balance {
  accrued_funding: SignedUint
  closing_fee: SignedUint
  opening_fee: SignedUint
  price_pnl: SignedUint
  total: SignedUint
}
export interface CashFlow {
  accrued_funding: SignedUint
  closing_fee: SignedUint
  opening_fee: SignedUint
  price_pnl: SignedUint
}
export interface PnlAmounts {
  accrued_funding: SignedUint
  closing_fee: SignedUint
  opening_fee: SignedUint
  pnl: SignedUint
  price_pnl: SignedUint
}
export interface DenomStateResponse {
  denom: string
  enabled: boolean
  funding: Funding
  last_updated: number
  total_cost_base: SignedUint
}
export interface Funding {
  last_funding_accrued_per_unit_in_base_denom: SignedDecimal
  last_funding_rate: SignedDecimal
  max_funding_velocity: Decimal
  skew_scale: Uint128
}
export interface SignedDecimal {
  abs: Decimal
  negative: boolean
  [k: string]: unknown
}
export type ArrayOfDenomStateResponse = DenomStateResponse[]
export interface PerpVaultDeposit {
  amount: Uint128
  shares: Uint128
}
export interface TradingFee {
  fee: Coin
  rate: Decimal
}
export interface Coin {
  amount: Uint128
  denom: string
  [k: string]: unknown
}
export interface OwnerResponse {
  abolished: boolean
  emergency_owner?: string | null
  initialized: boolean
  owner?: string | null
  proposed?: string | null
}
export interface PerpDenomState {
  denom: string
  enabled: boolean
  funding: Funding
  long_oi: Uint128
  pnl_values: PnlValues
  rate: SignedDecimal
  short_oi: Uint128
  total_entry_cost: SignedUint
  total_entry_funding: SignedUint
}
export interface PnlValues {
  accrued_funding: SignedUint
  closing_fee: SignedUint
  pnl: SignedUint
  price_pnl: SignedUint
}
export interface PaginationResponseForPerpDenomState {
  data: PerpDenomState[]
  metadata: Metadata
}
export interface Metadata {
  has_more: boolean
}
export type NullablePerpVaultPosition = PerpVaultPosition | null
export interface PerpVaultPosition {
  denom: string
  deposit: PerpVaultDeposit
  unlocks: PerpVaultUnlock[]
}
export interface PerpVaultUnlock {
  amount: Uint128
  cooldown_end: number
  created_at: number
  shares: Uint128
}
export interface PositionResponse {
  account_id: string
  position?: PerpPosition | null
}
export interface PerpPosition {
  base_denom: string
  closing_fee_rate: Decimal
  current_exec_price: Decimal
  current_price: Decimal
  denom: string
  entry_exec_price: Decimal
  entry_price: Decimal
  realised_pnl: PnlAmounts
  size: SignedUint
  unrealised_pnl: PnlAmounts
}
export interface PositionFeesResponse {
  base_denom: string
  closing_exec_price?: Decimal | null
  closing_fee: Uint128
  opening_exec_price?: Decimal | null
  opening_fee: Uint128
}
export type ArrayOfPositionResponse = PositionResponse[]
export interface PositionsByAccountResponse {
  account_id: string
  positions: PerpPosition[]
}
export type ArrayOfPerpVaultUnlock = PerpVaultUnlock[]
export interface VaultResponse {
  collateralization_ratio?: Decimal | null
  share_price?: Decimal | null
  total_balance: SignedUint
  total_debt: Uint128
  total_liquidity: Uint128
  total_shares: Uint128
  total_withdrawal_balance: Uint128
}
