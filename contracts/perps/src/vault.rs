use cosmwasm_std::{Deps, Uint128};
use mars_types::{adapters::oracle::Oracle, oracle::ActionKind, perps::VaultState};

use crate::{
    denom::compute_total_pnl,
    error::{ContractError, ContractResult},
};

const DEFAULT_SHARES_PER_AMOUNT: u128 = 1_000_000;

/// Compute the counterparty vault's net asset value (NAV), denominated in the
/// base asset (i.e. USDC).
///
/// The NAV is defined as
///
/// ```
/// NAV := max(assets - totalUnrealizedPnL, 0)
/// ```
///
/// Here `totalUnrealizedPnL` is the total unrealized PnL across _all_ denoms;
/// positive means traders are making gains, negative means traders are having
/// losses.
///
/// If a traders has an unrealized gain, it's a liability for the counterparty
/// vault, because if the user realizes the position it will be the vault to pay
/// for the profit.
///
/// Conversely, to realize a losing position the user must pay the vault, so
/// it's an asset for the vault.
///
/// We don't consider funding fees in this computation, because funding fees are
/// paid by one group of traders to another, so the net effect on NAV should be
/// zero.
//
// TODO: We might need to consider position opening/closing fees too, but right
// now we haven't decided how these fees will be implemented.
//
// TODO: Currently this is very gas-expensive, because we have to loop through
// all denoms, and for each denom we have to query the oracle contract for the
// current price.
// A possible optimization is this - each time the oracle price is updated, we
// recalculate the total PnL and cache it here. Then we only need to load the
// cached value.
pub fn compute_nav(
    deps: Deps,
    base_denom: &str,
    oracle: &Oracle,
    vs: &VaultState,
    current_time: u64,
) -> ContractResult<Uint128> {
    // loop through denoms and compute the total PnL
    // note: this PnL is denominated in USD
    let total_pnl = compute_total_pnl(deps, oracle, current_time)?.pnl;

    // convert the PnL to base currency (USDC)
    let base_price = oracle.query_price(&deps.querier, base_denom, ActionKind::Default)?.price;
    let total_pnl_in_base_currency = total_pnl.checked_div(base_price.into())?;

    // NAV := max(assets - totalUnrealizedPnL, 0)
    let nav = if total_pnl_in_base_currency.is_positive() {
        vs.total_liquidity.saturating_sub(total_pnl_in_base_currency.abs.to_uint_ceil())
    } else {
        vs.total_liquidity.checked_add(total_pnl_in_base_currency.abs.to_uint_floor())?
    };

    Ok(nav)
}

/// Convert a deposit amount to shares, given the current total amount and
/// shares.
///
/// If total shares is zero, in which case a conversion rate between amount and
/// shares is undefined, we use a default conversion rate.
pub fn amount_to_shares(vs: &VaultState, amount: Uint128) -> ContractResult<Uint128> {
    if vs.total_shares.is_zero() {
        return amount.checked_mul(Uint128::new(DEFAULT_SHARES_PER_AMOUNT)).map_err(Into::into);
    }

    // TODO: use NAV instead of vs.total_liquidity
    vs.total_shares.checked_multiply_ratio(amount, vs.total_liquidity).map_err(Into::into)
}

/// Convert a deposit shares to amount, given the current total amount and
/// shares.
///
/// If total shares is zero, in which case a conversion rate between amount and
/// shares if undefined, we throw an error.
pub fn shares_to_amount(vs: &VaultState, shares: Uint128) -> ContractResult<Uint128> {
    // We technical don't need to check for this explicitly, because
    // checked_multiply_raio already checks for division-by-zero. However we
    // still do this to output a more descriptive error message. This consumes a
    // bit more gas but gas fee is not yet a problem on Cosmos chains anyways.
    if vs.total_shares.is_zero() {
        return Err(ContractError::ZeroTotalShares);
    }

    // TODO: use NAV instead of vs.total_liquidity
    vs.total_liquidity.checked_multiply_ratio(shares, vs.total_shares).map_err(Into::into)
}
