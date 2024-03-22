use std::{cmp::min, collections::HashMap, str::FromStr};

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Coin, Decimal, Fraction, Uint128};
use mars_types::{
    credit_manager::Positions,
    health::{
        AccountKind, BorrowTarget, Health,
        HealthError::{
            MissingAmount, MissingAssetParams, MissingDenomState, MissingHLSParams,
            MissingPerpParams, MissingPrice, MissingVaultConfig, MissingVaultValues,
        },
        HealthResult, LiquidationPriceKind, SwapKind,
    },
    math::SignedDecimal,
    params::{AssetParams, CmSettings, VaultConfig},
    perps::{PerpPosition, PnL},
};
#[cfg(feature = "javascript")]
use tsify::Tsify;

use crate::{
    utils::calculate_remaining_oi_value, CollateralValue, PerpHealthFactorValues, PerpPnlValues,
    PerpsData, VaultsData,
};

/// `HealthComputer` is a shared struct with the frontend that gets compiled to wasm.
/// For this reason, it uses a dependency-injection-like pattern where all required data is needed up front.
#[cw_serde]
#[cfg_attr(feature = "javascript", derive(Tsify))]
#[cfg_attr(feature = "javascript", tsify(into_wasm_abi, from_wasm_abi))]
pub struct HealthComputer {
    pub kind: AccountKind,
    pub positions: Positions,
    pub asset_params: HashMap<String, AssetParams>,
    pub vaults_data: VaultsData,
    pub perps_data: PerpsData,
    pub oracle_prices: HashMap<String, Decimal>,
}

#[cw_serde]
#[cfg_attr(feature = "javascript", derive(Tsify))]
#[cfg_attr(feature = "javascript", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Direction {
    Long,
    Short,
}

impl Direction {
    pub fn sign(&self) -> SignedDecimal {
        match self {
            Direction::Long => SignedDecimal::one(),
            Direction::Short => SignedDecimal {
                negative: true,
                abs: Decimal::one(),
            },
        }
    }
}

impl HealthComputer {
    pub fn compute_health(&self) -> HealthResult<Health> {
        let CollateralValue {
            total_collateral_value,
            max_ltv_adjusted_collateral,
            liquidation_threshold_adjusted_collateral,
        } = self.total_collateral_value()?;

        let liquidation_threshold_adjusted_collateral_dec: SignedDecimal =
            liquidation_threshold_adjusted_collateral.into();
        let max_ltv_adjusted_collateral_dec: SignedDecimal = max_ltv_adjusted_collateral.into();
        let spot_debt_value: SignedDecimal = self.spot_debt_value()?.into();

        let perp_hf_values = self.perp_health_factor_values(&self.positions.perps)?;

        let (max_ltv_health_factor, liquidation_health_factor) =
            if spot_debt_value.is_zero() && self.positions.perps.is_empty() {
                (None, None)
            } else {
                // NOTE : The HF calc in the latest doc (0.9) differs slightly from this implementation.
                // reason being that risk team is still deciding on the correctness of
                // that formula.
                // The difference is in how funding is applied.
                // Currently, we include usdc collateral as part of RWA and apply f+ / f- to each perp position
                // The document uses C+, C- instead.
                // HF = (RWA + perp_numerator) / (spot_debt + perp_denominator)
                // where
                // RWA = risk weighted assets (i.e ltv * collateral_value)
                // spot debt = total value of borrowed assets (does not include perp unrealised pnl)

                let max_ltv_hf = Decimal::checked_from_ratio(
                    max_ltv_adjusted_collateral_dec
                        .checked_add(perp_hf_values.max_ltv_numerator)?
                        .abs
                        .to_uint_floor(),
                    spot_debt_value
                        .checked_add(perp_hf_values.max_ltv_denominator)?
                        .abs
                        .to_uint_floor(),
                )?;
                let liq_hf = Decimal::checked_from_ratio(
                    liquidation_threshold_adjusted_collateral_dec
                        .checked_add(perp_hf_values.liq_ltv_numerator)?
                        .abs
                        .to_uint_floor(),
                    spot_debt_value
                        .checked_add(perp_hf_values.liq_ltv_denominator)?
                        .abs
                        .to_uint_floor(),
                )?;
                (Some(max_ltv_hf), Some(liq_hf))
            };

        Ok(Health {
            total_debt_value: spot_debt_value.abs.to_uint_floor(),
            total_collateral_value,
            max_ltv_adjusted_collateral,
            liquidation_threshold_adjusted_collateral,
            max_ltv_health_factor,
            liquidation_health_factor,
            perp_pnl_profit: perp_hf_values.pnl_values.profit,
            perp_pnl_losses: perp_hf_values.pnl_values.loss,
        })
    }

    /// The max this account can withdraw of `withdraw_denom` and maintain max_ltv >= 1
    /// Note: This is an estimate. Guarantees to leave account healthy, but in edge cases,
    /// due to rounding, it may be slightly too conservative.
    pub fn max_withdraw_amount_estimate(&self, withdraw_denom: &str) -> HealthResult<Uint128> {
        // Both deposits and lends should be considered, as the funds can automatically be un-lent and
        // and also used to withdraw.
        let withdraw_coin = self.get_coin_from_deposits_and_lends(withdraw_denom)?;
        if withdraw_coin.amount.is_zero() {
            return Ok(Uint128::zero());
        };

        let params = self
            .asset_params
            .get(withdraw_denom)
            .ok_or(MissingAssetParams(withdraw_denom.to_string()))?;

        // If no debt or coin is blacklisted (meaning does not contribute to max ltv hf),
        // the total amount deposited can be withdrawn
        if (self.positions.debts.is_empty() && self.positions.perps.is_empty())
            || !params.credit_manager.whitelisted
        {
            return Ok(withdraw_coin.amount);
        }

        // withdraw denom max ltv adjusted value = total max ltv adjusted value - debt value - perp_denominator + perp_numerator
        let total_max_ltv_adjusted_value: SignedDecimal =
            self.total_collateral_value()?.max_ltv_adjusted_collateral.into();
        let debt_value: SignedDecimal = self.spot_debt_value()?.into();

        let withdraw_denom_price = *self
            .oracle_prices
            .get(withdraw_denom)
            .ok_or(MissingPrice(withdraw_denom.to_string()))?;

        let withdraw_denom_max_ltv = match self.kind {
            AccountKind::Default => params.max_loan_to_value,
            AccountKind::HighLeveredStrategy => {
                params
                    .credit_manager
                    .hls
                    .as_ref()
                    .ok_or(MissingHLSParams(withdraw_denom.to_string()))?
                    .max_loan_to_value
            }
        };

        let PerpHealthFactorValues {
            max_ltv_denominator: perp_denominator,
            max_ltv_numerator: perp_numerator,
            ..
        } = self.perp_health_factor_values(&self.positions.perps)?;

        // We often add one to calcs for a margin of error
        let one = SignedDecimal::one();

        // If we have any perps or debt, we need to check our health before continuing
        if !self.positions.perps.is_empty() || debt_value.abs > Decimal::zero() {
            let hf = total_max_ltv_adjusted_value
                .checked_add(perp_numerator)?
                .checked_div(debt_value.checked_add(perp_denominator)?)?;

            // Zero borrowable if unhealthy
            if hf.abs.le(&one.abs) {
                return Ok(Uint128::zero());
            }
        }

        // The max withdraw amount is calculated as:
        // withdraw denom max ltv adjusted value = total max ltv adjusted value - debt value - perp_denominator + perp_numerator
        let max_withdraw_value = total_max_ltv_adjusted_value
            .checked_sub(debt_value)?
            .checked_sub(perp_denominator)?
            .checked_add(perp_numerator)?
            .checked_sub(one)?
            .abs
            .to_uint_floor();

        // The above is the raw value, now we need to factor in price and LTV impact
        let max_withdraw_amount = max_withdraw_value
            .checked_div_floor(withdraw_denom_price.checked_mul(withdraw_denom_max_ltv)?)?;

        Ok(min(max_withdraw_amount, withdraw_coin.amount))
    }

    pub fn max_swap_amount_estimate(
        &self,
        from_denom: &str,
        to_denom: &str,
        kind: &SwapKind,
        slippage: Decimal,
    ) -> HealthResult<Uint128> {
        // Both deposits and lends should be considered, as the funds can automatically be un-lent and
        // and also used to swap.
        let from_coin = self.get_coin_from_deposits_and_lends(from_denom)?;

        // If no debt the total amount deposited can be swapped (only for default swaps)
        if kind == &SwapKind::Default
            && self.positions.debts.is_empty()
            && self.positions.perps.is_empty()
        {
            return Ok(from_coin.amount);
        }

        let total_max_ltv_adjusted_value: SignedDecimal =
            self.total_collateral_value()?.max_ltv_adjusted_collateral.into();

        let debt_value: SignedDecimal = self.spot_debt_value()?.into();

        let PerpHealthFactorValues {
            max_ltv_denominator: perp_denominator,
            max_ltv_numerator: perp_numerator,
            ..
        } = self.perp_health_factor_values(&self.positions.perps)?;

        let one = SignedDecimal::one();

        if !self.positions.perps.is_empty() || debt_value.abs > Decimal::zero() {
            let hf = total_max_ltv_adjusted_value
                .checked_add(perp_numerator)?
                .checked_div(debt_value.checked_add(perp_denominator)?)?;

            // Zero borrowable if unhealthy
            if hf.abs.le(&one.abs) {
                return Ok(Uint128::zero());
            }
        }

        let from_ltv = self.get_coin_max_ltv(from_denom)?;
        let to_ltv = self.get_coin_max_ltv(to_denom)?;

        // Don't allow swapping when one of the assets is not whitelisted
        if from_ltv == Decimal::zero() || to_ltv == Decimal::zero() {
            return Ok(Uint128::zero());
        }

        let from_price =
            self.oracle_prices.get(from_denom).ok_or(MissingPrice(from_denom.to_string()))?;

        // An asset that has a price of 1 and max ltv of 0.5 has a collateral_value of 0.5.
        // Swapping that asset for an asset with the same price, but 0.8 max ltv results in a collateral_value of 0.8.
        // Therefore, when the asset that is swapped to has a higher or equal max ltv than the asset swapped from,
        // the collateral value will increase and we can allow the full balance to be swapped.
        // The ltv_out is adjusted for slippage, as the swap_out_value can drop by the slippage.
        let to_ltv_slippage_corrected = to_ltv.checked_mul(Decimal::one() - slippage)?;
        let swappable_amount = if to_ltv_slippage_corrected >= from_ltv {
            from_coin.amount
        } else {
            // In order to calculate the output of the swap, the formula looks like this:
            //     1 = (collateral_value + to_amount * to_price * to_ltv - from_amount * from_price * from_ltv) / debt_value
            // The unknown variables here are to_amount and from_amount. In order to only have 1 unknown variable, from_amount,
            // to_amount can be replaced by:
            //     to_amount = slippage * from_amount * from_price / to_price
            // This results in the following formula:
            //     1 = (collateral_value + slippage * from_amount * from_price / to_price * to_price * to_ltv - from_amount * from_price * from_ltv) / debt_value
            //     debt_value = collateral_value + slippage * from_amount * from_price * to_ltv - from_amount * from_price * from_ltv
            //     slippage * from_amount * from_price * to_ltv - from_amount * from_price * from_ltv = debt_value - collateral_value
            //     from_amount * (slippage * from_price * to_ltv - from_price * from_ltv) = debt_value - collateral_value
            // Rearranging this formula to isolate from_amount results in the following formula:
            //     from_amount = (debt_value - collateral_value) / (from_price * (slippage * to_ltv - from_ltv))
            // Rearranging to avoid negative numbers for the denominator (to_ltv_slippage_corrected < from_ltv):
            //     from_amount = (collateral_value - debt_value) / (from_price * (from_ltv - slippage * to_ltv)
            // Rearranging to include perp values:
            //    from_amount = (collateral_value - debt_value - perpd + perpn) / (from_price * (from_ltv - slippage * to_ltv)
            let amount = total_max_ltv_adjusted_value
                .checked_sub(debt_value)?
                .checked_sub(perp_denominator)?
                .checked_add(perp_numerator)?
                .checked_sub(one)?
                .abs
                .to_uint_floor() // Uint128 is used to avoid overflows in the division with Decimals
                .checked_div_floor(from_price.checked_mul(from_ltv - to_ltv_slippage_corrected)?)?;

            // Cap the swappable amount at the current balance of the coin
            min(amount, from_coin.amount)
        };

        match kind {
            SwapKind::Default => Ok(swappable_amount),

            SwapKind::Margin => {
                // If the swappable amount is less than the available amount, no need to further calculate
                // the margin borrow amount.
                if swappable_amount < from_coin.amount {
                    return Ok(swappable_amount);
                }

                let from_coin_value = from_coin.amount.checked_mul_floor(*from_price)?;

                // This represents the max ltv adjusted value of the coin being swapped from
                let swap_from_ltv_value = from_coin_value.checked_mul_floor(from_ltv)?;

                // The from_denom is always taken on as debt, as the trade is the bullish direction
                // of the to_denom (expecting it to outpace the borrow rate from the from_denom)
                let swap_to_ltv_value = from_coin_value.checked_mul_floor(to_ltv)?;

                let total_max_ltv_adjust_value_after_swap = total_max_ltv_adjusted_value
                    .checked_add(SignedDecimal::from(swap_to_ltv_value))?
                    .checked_sub(SignedDecimal::from(swap_from_ltv_value))?;

                // The total swappable amount for margin is represented by the available coin balance + the
                // the maximum amount that can be borrowed (and then swapped).
                // This is represented by the formula:
                //     1 = (collateral_after_swap + slippage * borrow_amount * borrow_price * to_ltv) / (debt + borrow_amount * borrow_price)
                //     debt + borrow_amount * borrow_price = collateral_after_swap + slippage * borrow_amount * borrow_price * to_ltv
                //     borrow_amount * borrow_price - slippage * borrow_amount * borrow_price * to_ltv = collateral_after_swap - debt
                //     borrow_amount * borrow_price * (1 - slippage * to_ltv) = collateral_after_swap - debt
                // Rearranging this results in:
                //     borrow_amount = (collateral_after_swap - debt) / (borrow_price * (1 - slippage * to_ltv))
                // Rearranging to include perp values:
                //    borrow_amount = (collateral_after_swap - debt - perpd + perpn) / (borrow_price * (1 - slippage * to_ltv))
                let borrow_amount = total_max_ltv_adjust_value_after_swap
                    .checked_sub(debt_value)?
                    .checked_sub(perp_denominator)?
                    .checked_add(perp_numerator)?
                    .checked_sub(one)?
                    .abs
                    .to_uint_floor() // Uint128 is used to avoid overflows in the division with Decimals
                    .checked_div_floor(
                        Decimal::one()
                            .checked_sub(to_ltv_slippage_corrected)?
                            .checked_mul(*from_price)?,
                    )?;

                // The total amount that can be swapped is then the balance of the coin + the additional amount
                // that can be borrowed.
                Ok(borrow_amount.checked_add(from_coin.amount)?)
            }
        }
    }

    /// The max this account can borrow of `borrow_denom` and maintain max_ltv >= 1
    /// Note: This is an estimate. Guarantees to leave account healthy, but in edge cases,
    /// due to rounding, it may be slightly too conservative.
    pub fn max_borrow_amount_estimate(
        &self,
        borrow_denom: &str,
        target: &BorrowTarget,
    ) -> HealthResult<Uint128> {
        let total_max_ltv_adjusted_value: SignedDecimal =
            self.total_collateral_value()?.max_ltv_adjusted_collateral.into();
        let debt_value: SignedDecimal = self.spot_debt_value()?.into();

        // We often add one to calcs for a margin of error, so rather than create it multiple times we just create it once here.
        let one = SignedDecimal::one();

        // Perp values
        let PerpHealthFactorValues {
            max_ltv_denominator: perp_denominator,
            max_ltv_numerator: perp_numerator,
            ..
        } = self.perp_health_factor_values(&self.positions.perps)?;

        let params = self
            .asset_params
            .get(borrow_denom)
            .ok_or(MissingAssetParams(borrow_denom.to_string()))?;

        // If asset not whitelisted we cannot borrow
        if !params.credit_manager.whitelisted || total_max_ltv_adjusted_value.is_zero() {
            return Ok(Uint128::zero());
        }

        // If we have perp positions or debt we need to check if the health factor is above 1
        if !self.positions.perps.is_empty() || debt_value.abs > Decimal::zero() {
            let hf = total_max_ltv_adjusted_value
                .checked_add(perp_numerator)?
                .checked_div(debt_value.checked_add(perp_denominator)?)?;

            // Zero borrowable if unhealthy
            if hf.abs.le(&one.abs) {
                return Ok(Uint128::zero());
            }
        }

        let borrow_denom_max_ltv = match self.kind {
            AccountKind::Default => params.max_loan_to_value,
            AccountKind::HighLeveredStrategy => {
                params
                    .credit_manager
                    .hls
                    .as_ref()
                    .ok_or(MissingHLSParams(borrow_denom.to_string()))?
                    .max_loan_to_value
            }
        };

        let borrow_denom_price = self
            .oracle_prices
            .get(borrow_denom)
            .cloned()
            .ok_or(MissingPrice(borrow_denom.to_string()))?;

        // The formulas look like this in practice:
        //      hf = rounddown(roundown(amount * price) * perp_numerator) / (spot_debt value + perp_denominator)
        // Which means re-arranging this to isolate borrow amount is an estimate,
        // quite close, but never precisely right. For this reason, the + 1 of the formulas
        // below are meant to err on the side of being more conservative vs aggressive.

        let max_borrow_amount = match target {
            // The max borrow for deposit can be calculated as:
            //      1 = (max ltv adjusted value + (borrow denom amount * borrow denom price * borrow denom max ltv) + perpn) / (debt value + (borrow denom amount * borrow denom price) + perpd)
            // Re-arranging this to isolate borrow denom amount renders:
            //      max_borrow_denom_amount = max ltv adjusted value - debt value - perpd + perpn / (borrow_denom_price * (1 - borrow_denom_max_ltv)))
            BorrowTarget::Deposit => {
                let numerator = total_max_ltv_adjusted_value
                    .checked_sub(debt_value)?
                    .checked_sub(perp_denominator)?
                    .checked_add(perp_numerator)?
                    .checked_sub(one)?
                    .abs
                    .to_uint_floor();

                let denominator =
                    borrow_denom_price.checked_mul(one.abs.checked_sub(borrow_denom_max_ltv)?)?;

                // It is important to use Uint128 and Decimal types to avoid overflows. This way we can ensure that
                // after the division we get the correct value as a Uint128 (we are rounding down and removing extra numbers after comma).
                // If we use Decimals for numerator and denominator directly, we can get an overflow and the result will be incorrect.
                numerator.checked_div_floor(denominator)?
            }

            // Borrowing assets to wallet does not count towards collateral. It only adds to debts.
            // Hence, the max borrow to wallet can be calculated as:
            //      1 = (max ltv adjusted value) + perpn / (debt value + (borrow denom amount * borrow denom price)) + perpd
            // Re-arranging this to isolate borrow denom amount renders:
            //      borrow denom amount = (max ltv adjusted value - debt_value - perpd + perpn) / denom_price
            BorrowTarget::Wallet => {
                let numerator = total_max_ltv_adjusted_value
                    .checked_sub(debt_value)?
                    .checked_sub(perp_denominator)?
                    .checked_add(perp_numerator)?
                    .checked_sub(one)?
                    .abs
                    .to_uint_floor();

                // It is important to use Uint128 and Decimal types to avoid overflows. This way we can ensure that
                // after the division we get the correct value as a Uint128 (we are rounding down and removing extra numbers after comma).
                // If we use Decimals for numerator and denominator directly, we can get an overflow and the result will be incorrect.
                numerator.checked_div_floor(borrow_denom_price)?
            }

            // When borrowing assets to add to a vault, the amount deposited into the vault counts towards collateral.
            // The health factor can be calculated as:
            //     1 = (max ltv adjusted value + (borrow amount * borrow price * vault max ltv)) / (debt value + (borrow amount * borrow price))
            // Re-arranging this to isolate borrow amount renders:
            //     borrow amount = (max ltv adjusted value - debt value + perpd - perpn) / (borrow price * (1 - vault max ltv)
            BorrowTarget::Vault {
                address,
            } => {
                let VaultConfig {
                    addr,
                    max_loan_to_value,
                    whitelisted,
                    hls,
                    ..
                } = self
                    .vaults_data
                    .vault_configs
                    .get(address)
                    .ok_or(MissingVaultConfig(address.to_string()))?;

                // If vault or base token has been de-listed, drop MaxLTV to zero
                let checked_vault_max_ltv = if *whitelisted {
                    match self.kind {
                        AccountKind::Default => *max_loan_to_value,
                        AccountKind::HighLeveredStrategy => {
                            hls.as_ref()
                                .ok_or(MissingHLSParams(addr.to_string()))?
                                .max_loan_to_value
                        }
                    }
                } else {
                    Decimal::zero()
                };

                // The max borrow for deposit can be calculated as:
                //      1 = (total_max_ltv_adjusted_value + (max_borrow_denom_amount * borrow_denom_price * checked_vault_max_ltv) + perpn) / (debt_value + (max_borrow_denom_amount * borrow_denom_price)) + perpd
                // Re-arranging this to isolate borrow denom amount renders:
                //      max_borrow_denom_amount = (total_max_ltv_adjusted_value-debt_value + perpn - perpd) / (borrow_denom_price * (1 - checked_vault_max_ltv))
                // Which means re-arranging this to isolate borrow amount is an estimate,
                // quite close, but never precisely right. For this reason, the - 1 of the formulas
                // below are meant to err on the side of being more conservative vs aggressive.
                let numerator = total_max_ltv_adjusted_value
                    .checked_sub(debt_value)?
                    .checked_sub(perp_denominator)?
                    .checked_add(perp_numerator)?
                    .checked_sub(one)?
                    .abs
                    .to_uint_floor();

                let denominator =
                    borrow_denom_price.checked_mul(one.abs.checked_sub(checked_vault_max_ltv)?)?;

                // It is important to use Uint128 and Decimal types to avoid overflows. This way we can ensure that
                // after the division we get the correct value as a Uint128 (we are rounding down and removing extra numbers after comma).
                // If we use Decimals for numerator and denominator directly, we can get an overflow and the result will be incorrect.
                numerator.checked_div_floor(denominator)?
            }

            BorrowTarget::Swap {
                slippage,
                denom_out,
            } => {
                let denom_out_ltv = self.get_coin_max_ltv(denom_out).unwrap();

                // The max borrow for swap can be calculated as:
                //      1 = (total_max_ltv_adjusted_value + (denom_amount_out * denom_price_out * denom_out_ltv)) / (debt_value + (max_borrow_denom_amount * borrow_denom_price))
                // denom_amount_out can be replaced by:
                //      denom_amount_out = slippage * max_borrow_denom_amount * borrow_denom_price / denom_price_out
                // This results in the following formula:
                //      1 = (total_max_ltv_adjusted_value + (slippage * max_borrow_denom_amount * borrow_denom_price * denom_out_ltv)) / (debt_value + (max_borrow_denom_amount * borrow_denom_price))
                // Re-arranging this to isolate borrow denom amount renders:
                //      max_borrow_denom_amount = (total_max_ltv_adjusted_value - debt_value) / (borrow_denom_price * (1 - slippage * denom_out_ltv))
                // Re-arranging to include perp values:
                //      max_borrow_denom_amount = (total_max_ltv_adjusted_value - debt_value - perpd + perpn) / (borrow_denom_price * (1 - slippage * denom_out_ltv))
                let out_ltv_slippage_corrected =
                    denom_out_ltv.checked_mul(Decimal::one() - slippage)?;

                let numerator = total_max_ltv_adjusted_value
                    .checked_sub(debt_value)?
                    .checked_sub(perp_denominator)?
                    .checked_add(perp_numerator)?
                    .checked_sub(one)?
                    .abs
                    .to_uint_floor();

                let denominator = borrow_denom_price
                    .checked_mul(one.abs.checked_sub(out_ltv_slippage_corrected)?)?;

                // It is important to use Uint128 and Decimal types to avoid overflows. This way we can ensure that
                // after the division we get the correct value as a Uint128 (we are rounding down and removing extra numbers after comma).
                // If we use Decimals for numerator and denominator directly, we can get an overflow and the result will be incorrect.
                numerator.checked_div_floor(denominator)?
            }
        };

        Ok(max_borrow_amount)
    }

    /// Estimate the max long and short size that our user can take.
    /// The max position size can be calculated as: - (b+sqr(d)) / (2*a)
    pub fn max_perp_size_estimate(
        &self,
        denom: &str,
        base_denom: &str,
        long_oi_amount: Decimal,
        short_oi_amount: Decimal,
        direction: &Direction,
    ) -> HealthResult<SignedDecimal> {
        // Constant
        let two = SignedDecimal::from_str("2")?;

        // prices
        let perp_oracle_price: SignedDecimal =
            (*self.oracle_prices.get(denom).ok_or(MissingPrice(denom.to_string()))?).into();
        let base_denom_price: SignedDecimal =
            (*self.oracle_prices.get(base_denom).ok_or(MissingPrice(base_denom.to_string()))?)
                .into();

        // Denom state
        let denom_state =
            self.perps_data.denom_states.get(denom).ok_or(MissingDenomState(denom.to_string()))?;

        // Perp market params
        let perp_params =
            self.perps_data.params.get(denom).ok_or(MissingPerpParams(denom.to_string()))?;
        let closing_fee_rate = perp_params.closing_fee_rate;
        let opening_fee_rate = perp_params.opening_fee_rate;
        let skew_scale: SignedDecimal = denom_state.funding.skew_scale.into();
        let ltv_base_denom = self.get_coin_max_ltv(base_denom)?;
        let ltv_p: SignedDecimal = perp_params.max_loan_to_value.into();

        // The max position change amount afforded by the open interest caps, in the given direction
        let max_oi_change_amount = calculate_remaining_oi_value(
            long_oi_amount,
            short_oi_amount,
            perp_oracle_price.abs,
            perp_params,
            direction,
        )?;

        if max_oi_change_amount.is_zero() {
            return Ok(max_oi_change_amount);
        }
        // Current skew
        let k = SignedDecimal::from(long_oi_amount).checked_sub(short_oi_amount.into())?;

        let (
            // Current unrealised funding
            f,
            // Current size,
            q_old,
            // Entry price
            p_ex_o,
        ) = self.positions.perps.iter().find(|&x| x.denom == *denom).map_or(
            (
                SignedDecimal::zero(),
                SignedDecimal::zero(),
                self.get_execution_price(perp_oracle_price, k, skew_scale, SignedDecimal::zero())?,
            ),
            |f| (f.unrealised_pnl.values.accrued_funding, f.size, f.entry_exec_price.into()),
        );

        let p_ex = self.get_execution_price(perp_oracle_price, k, skew_scale, q_old)?;
        let closing_fee_value = p_ex.abs.checked_mul(closing_fee_rate)?.checked_mul(q_old.abs)?;

        // Indicator functions
        let (i, i_prim) = if (q_old.is_negative() && direction == &Direction::Long)
            || (q_old.is_positive() && direction == &Direction::Short)
        {
            // opposite direction
            (Decimal::zero(), Decimal::one())
            // Same direction
        } else {
            (Decimal::one(), Decimal::zero())
        };

        let u_pnl = match q_old.is_zero() {
            true => SignedDecimal::zero(),
            false => q_old.checked_mul(p_ex.checked_sub(p_ex_o)?)?.checked_add(f)?,
        };

        let (base_denom_collateral_value, rwa_value, debt_value) =
            self.account_composition(base_denom, denom, base_denom_price)?;

        // z = LTVp - closing fee - opening fee - 1
        let z: SignedDecimal = ltv_p
            .checked_sub(closing_fee_rate.into())?
            .checked_sub(opening_fee_rate.into())?
            .checked_sub(SignedDecimal::one())?;

        // a = - z * (price_oracle / (2 * skew_scale)) (SHORT)
        // a = z * (price_oracle / (2 * skew_scale)) (LONG)
        let mut a = z.checked_mul(perp_oracle_price.checked_div(two.checked_mul(skew_scale)?)?)?;
        a = a.checked_mul(direction.sign())?;

        // b = z * price_oracle * (1 + (k - q_old / 2) / skew_scale)
        let b = perp_oracle_price.checked_mul(z)?.checked_mul(
            SignedDecimal::one().checked_add(k.checked_sub(q_old)?.checked_div(skew_scale)?)?,
        )?;

        // c = based_denom_value + u_pnl - closing_fee_value * i_prim
        let c = base_denom_collateral_value
            .checked_add(u_pnl.checked_sub(closing_fee_value.checked_mul(i_prim)?.into())?)?;

        // c+ = max(0, c)
        let c_max = SignedDecimal::zero().max(c);

        // c- = -min(0, c)
        let c_min = SignedDecimal::zero().checked_sub(SignedDecimal::zero().min(c))?;

        // c_delta = (c_max * LTV_base_denom) - c_min
        let c_delta = c_max.checked_mul(ltv_base_denom.into())?.checked_sub(c_min)?;

        // C_add = price_oracle * |q_old| * opening_fee_rate * (1 + (k - q_old / 2) / skew_scale) * i
        let c_add =
            perp_oracle_price
                .checked_mul(q_old.abs.into())?
                .checked_mul(opening_fee_rate.into())?
                .checked_mul(SignedDecimal::from_str("1")?.checked_add(
                    k.checked_sub(q_old.checked_div(two)?)?.checked_div(skew_scale)?,
                )?)?
                .checked_mul(i.into())?;

        // c = RWA - debt + c_delta + c_add
        let c = rwa_value.checked_sub(debt_value)?.checked_add(c_delta)?.checked_add(c_add)?;

        // d = b^2 - 4ac
        let d = b
            .checked_mul(b)?
            .checked_sub(SignedDecimal::from_str("4")?.checked_mul(a)?.checked_mul(c)?)?;

        // q_max = - (b + sqrt(d)) / (2 * a)
        let mut q_max_amount = SignedDecimal::zero()
            .checked_sub(b.checked_add(d.abs.sqrt().into())?.checked_div(two.checked_mul(a)?)?)?;

        q_max_amount = if q_max_amount.abs > max_oi_change_amount.abs {
            max_oi_change_amount
        } else {
            q_max_amount
        };

        q_max_amount = if direction == &Direction::Long {
            q_max_amount
        } else {
            SignedDecimal::zero().checked_sub(q_max_amount)?
        };

        Ok(q_max_amount)
    }

    // TODO this calc seems to be functionally equivilent to the execution_closing_price in perps::pricing.
    // We should look to extract to a common helper method
    fn get_execution_price(
        &self,
        perp_oracle_price: SignedDecimal,
        skew: SignedDecimal,
        skew_scale: SignedDecimal,
        q_old: SignedDecimal,
    ) -> HealthResult<SignedDecimal> {
        let subtractor = if q_old.is_zero() {
            SignedDecimal::zero()
        } else {
            q_old.checked_div(SignedDecimal::from_str("2")?)?
        };

        Ok(perp_oracle_price.checked_mul(
            SignedDecimal::one()
                .checked_add(skew.checked_sub(subtractor)?.checked_div(skew_scale)?)?,
        )?)
    }

    fn account_composition(
        &self,
        base_denom: &str,
        denom: &str,
        base_denom_price: SignedDecimal,
    ) -> HealthResult<(SignedDecimal, SignedDecimal, SignedDecimal)> {
        let (base_denom_deposits, other_deposits): (Vec<_>, Vec<_>) =
            self.positions.deposits.iter().partition(|deposit| deposit.denom == base_denom);

        // there is only one base denom deposit
        let account_base_denom_deposits =
            base_denom_deposits.first().map_or(Uint128::zero(), |d| d.amount);

        let (base_denom_lends, other_lends): (Vec<_>, Vec<_>) =
            self.positions.lends.iter().partition(|lend| lend.denom == base_denom);
        let account_base_denom_lends =
            base_denom_lends.first().map_or(Uint128::zero(), |l| l.amount);

        let filtered_perps: Vec<_> =
            self.positions.perps.iter().filter(|x| x.denom != denom).cloned().collect();

        // (named c_usdc in docs + sheet)
        // Refers to the value of collateral the user has in the base_denom (e.g usdc)
        let base_denom_collateral_value = base_denom_price.checked_mul(
            account_base_denom_deposits.checked_add(account_base_denom_lends)?.into(),
        )?;

        let deref_deposits: Vec<Coin> = other_deposits.into_iter().cloned().collect();
        let deref_lends: Vec<Coin> = other_lends.into_iter().cloned().collect();

        let assets_ltv_adjusted_value = self
            .coins_value(deref_deposits.as_slice())?
            .max_ltv_adjusted_collateral
            .checked_add(self.coins_value(deref_lends.as_slice())?.max_ltv_adjusted_collateral)?
            .checked_add(self.vaults_value()?.max_ltv_adjusted_collateral)?;

        // Contains denominator / numerator for HF for all perps *excluding* a perp position for given denom
        let other_perp_hf_values = self.perp_health_factor_values(&filtered_perps)?;

        // Risk Weighted Assets (rwa) are assets other than base_denom and the perp position being considered, weighted using corresponding Maximum LTVs
        let other_collateral_value: SignedDecimal = SignedDecimal::from(assets_ltv_adjusted_value)
            .checked_add(other_perp_hf_values.max_ltv_numerator)?;

        // raw_debt = all debt and everything from the denominator of perps besides
        // the position for given denom.
        let mut raw_debt_value = Uint128::zero();

        for d in &self.positions.debts {
            let price = self
                .oracle_prices
                .get(&d.denom)
                .ok_or_else(|| MissingPrice(d.denom.to_string()))?;

            let product = d.amount.checked_mul_ceil(*price)?;
            raw_debt_value += product;
        }

        let sd_debt: SignedDecimal = raw_debt_value.into();

        // debt = raw_debt + max_ltv_denominator for perp positions *excluding* a perp position for given denom
        let debt_value: SignedDecimal =
            sd_debt.checked_add(other_perp_hf_values.max_ltv_denominator)?;

        Ok((base_denom_collateral_value, other_collateral_value, debt_value))
    }

    fn perp_health_factor_values(
        &self,
        perps: &[PerpPosition],
    ) -> HealthResult<PerpHealthFactorValues> {
        let mut max_ltv_numerator = SignedDecimal::zero();
        let mut max_ltv_denominator = SignedDecimal::zero();
        let mut liq_ltv_numerator = SignedDecimal::zero();
        let mut liq_ltv_denominator = SignedDecimal::zero();
        let mut profit = Uint128::zero();
        let mut loss = Uint128::zero();

        for position in perps.iter() {
            // Update our pnl values
            match &position.unrealised_pnl.coins.pnl {
                PnL::Profit(pnl) => profit = profit.checked_add(pnl.amount)?,
                PnL::Loss(pnl) => loss = loss.checked_add(pnl.amount)?,
                _ => {}
            }

            let denom = &position.denom;
            let base_denom = &position.base_denom;
            let base_denom_price: SignedDecimal = (*self
                .oracle_prices
                .get(base_denom)
                .ok_or(MissingPrice(base_denom.to_string()))?)
            .into();

            let (funding_min, funding_max) = self.get_min_and_max_funding_amounts(position)?;

            let funding_min_value = funding_min.checked_mul(base_denom_price)?;
            let funding_max_value = funding_max.checked_mul(base_denom_price)?;

            let closing_rate = position.closing_fee_rate.into();

            // Perp(0)
            let position_value_open: SignedDecimal =
                position.size.abs.checked_mul(position.entry_exec_price)?.into();
            let position_value_current: SignedDecimal =
                position.size.checked_mul(position.current_exec_price.into())?.abs.into();

            // Borrow and liquidation ltv maximums for the perp and the funding demom
            let checked_max_ltv: SignedDecimal = self.get_perp_max_ltv(denom)?.into();
            let checked_liq_ltv: SignedDecimal = self.get_perp_liq_ltv(denom)?.into();
            let checked_max_ltv_base_denom: SignedDecimal =
                self.get_coin_max_ltv(base_denom)?.into();
            let checked_liq_ltv_base_denom: SignedDecimal =
                self.get_liquidation_ltv(base_denom)?.into();

            // There are two different HF calculations, depending on if the perp
            // position is long or short.

            // For shorts, Health Factor = Perp(0) + (funding max accrued * base denom price * base denom ltv)  / (Perp (t) * (2 - MaxLTV + trading fee) + funding min * base denom price
            // For longs, Health Factor = (Perp (t) * (LTV-trading fee) + funding max * base denom price * base denom ltv  / Perp (t0) + funding min * base denom price
            // IF perp size is negative the position is short, positive long
            if position.size.is_negative() {
                // Numerator = position value(0) + (positive funding * base denom ltv * base denom price)
                let temp_ltv_numerator = position_value_open
                    .checked_add(funding_max_value.checked_mul(checked_max_ltv_base_denom)?)?;

                let temp_liq_numerator = position_value_open
                    .checked_add(funding_max_value.checked_mul(checked_liq_ltv_base_denom)?)?;

                // Denominator = position value(t) * (2 - max ltv + closing fee) + negative funding
                let temp_ltv_denominator = position_value_current
                    .checked_mul(
                        SignedDecimal::from_str("2.0")?
                            .checked_sub(checked_max_ltv)?
                            .checked_add(closing_rate)?,
                    )?
                    .checked_add(funding_min_value)?;

                let temp_liq_denominator = position_value_current
                    .checked_mul(
                        SignedDecimal::from_str("2.0")?
                            .checked_sub(checked_liq_ltv)?
                            .checked_add(closing_rate)?,
                    )?
                    .checked_add(funding_min_value)?;

                // Add values
                max_ltv_numerator = max_ltv_numerator.checked_add(temp_ltv_numerator)?;
                liq_ltv_numerator = liq_ltv_numerator.checked_add(temp_liq_numerator)?;
                max_ltv_denominator = max_ltv_denominator.checked_add(temp_ltv_denominator)?;
                liq_ltv_denominator = liq_ltv_denominator.checked_add(temp_liq_denominator)?;
            } else if position.size.is_positive() {
                // Numerator = position value(0) + (positive funding * base denom ltv)
                let temp_ltv_numerator = position_value_current
                    .checked_mul(checked_max_ltv.checked_sub(closing_rate)?)?
                    .checked_add(funding_max_value.checked_mul(checked_max_ltv_base_denom)?)?;

                let temp_liq_numerator = position_value_current
                    .checked_mul(checked_liq_ltv.checked_sub(closing_rate)?)?
                    .checked_add(funding_max_value.checked_mul(checked_liq_ltv_base_denom)?)?;

                // Denominator = position value(0) + negative funding
                let temp_denominator = position_value_open.checked_add(funding_min_value)?;

                // Add values
                max_ltv_numerator = max_ltv_numerator.checked_add(temp_ltv_numerator)?;
                liq_ltv_numerator = liq_ltv_numerator.checked_add(temp_liq_numerator)?;
                max_ltv_denominator = max_ltv_denominator.checked_add(temp_denominator)?;
                liq_ltv_denominator = liq_ltv_denominator.checked_add(temp_denominator)?;
            }

            // else perp size is zero - safe to do nothing? we should never get into this situation
            // but if we do we probably don't want to brick the HF calculation
        }

        Ok(PerpHealthFactorValues {
            max_ltv_numerator: max_ltv_numerator.floor(),
            max_ltv_denominator: max_ltv_denominator.floor(),
            liq_ltv_numerator: liq_ltv_numerator.floor(),
            liq_ltv_denominator: liq_ltv_denominator.floor(),
            pnl_values: PerpPnlValues {
                profit,
                loss,
            },
        })
    }

    fn total_collateral_value(&self) -> HealthResult<CollateralValue> {
        let deposits = self.coins_value(&self.positions.deposits)?;
        let lends = self.coins_value(&self.positions.lends)?;
        let vaults = self.vaults_value()?;

        Ok(CollateralValue {
            total_collateral_value: deposits
                .total_collateral_value
                .checked_add(vaults.total_collateral_value)?
                .checked_add(lends.total_collateral_value)?,
            max_ltv_adjusted_collateral: deposits
                .max_ltv_adjusted_collateral
                .checked_add(vaults.max_ltv_adjusted_collateral)?
                .checked_add(lends.max_ltv_adjusted_collateral)?,
            liquidation_threshold_adjusted_collateral: deposits
                .liquidation_threshold_adjusted_collateral
                .checked_add(vaults.liquidation_threshold_adjusted_collateral)?
                .checked_add(lends.liquidation_threshold_adjusted_collateral)?,
        })
    }

    fn coins_value(&self, coins: &[Coin]) -> HealthResult<CollateralValue> {
        let mut total_collateral_value = Uint128::zero();
        let mut max_ltv_adjusted_collateral = Uint128::zero();
        let mut liquidation_threshold_adjusted_collateral = Uint128::zero();

        for c in coins {
            let coin_price =
                self.oracle_prices.get(&c.denom).ok_or(MissingPrice(c.denom.clone()))?;
            let coin_value = c.amount.checked_mul_floor(*coin_price)?;
            total_collateral_value = total_collateral_value.checked_add(coin_value)?;

            let AssetParams {
                credit_manager:
                    CmSettings {
                        hls,
                        ..
                    },
                liquidation_threshold,
                ..
            } = self.asset_params.get(&c.denom).ok_or(MissingAssetParams(c.denom.clone()))?;

            let checked_max_ltv = self.get_coin_max_ltv(&c.denom)?;

            let max_ltv_adjusted = coin_value.checked_mul_floor(checked_max_ltv)?;
            max_ltv_adjusted_collateral =
                max_ltv_adjusted_collateral.checked_add(max_ltv_adjusted)?;

            let checked_liquidation_threshold = match self.kind {
                AccountKind::Default => *liquidation_threshold,
                AccountKind::HighLeveredStrategy => {
                    hls.as_ref().ok_or(MissingHLSParams(c.denom.clone()))?.liquidation_threshold
                }
            };
            let liq_adjusted = coin_value.checked_mul_floor(checked_liquidation_threshold)?;
            liquidation_threshold_adjusted_collateral =
                liquidation_threshold_adjusted_collateral.checked_add(liq_adjusted)?;
        }
        Ok(CollateralValue {
            total_collateral_value,
            max_ltv_adjusted_collateral,
            liquidation_threshold_adjusted_collateral,
        })
    }

    fn vaults_value(&self) -> HealthResult<CollateralValue> {
        let mut total_collateral_value = Uint128::zero();
        let mut max_ltv_adjusted_collateral = Uint128::zero();
        let mut liquidation_threshold_adjusted_collateral = Uint128::zero();

        for v in &self.positions.vaults {
            // Step 1: Calculate Vault coin values
            let values = self
                .vaults_data
                .vault_values
                .get(&v.vault.address)
                .ok_or(MissingVaultValues(v.vault.address.to_string()))?;

            total_collateral_value = total_collateral_value.checked_add(values.vault_coin.value)?;

            let VaultConfig {
                addr,
                max_loan_to_value,
                liquidation_threshold,
                whitelisted,
                hls,
                ..
            } = self
                .vaults_data
                .vault_configs
                .get(&v.vault.address)
                .ok_or(MissingVaultConfig(v.vault.address.to_string()))?;

            let base_params = self
                .asset_params
                .get(&values.base_coin.denom)
                .ok_or(MissingAssetParams(values.base_coin.denom.clone()))?;

            // If vault or base token has been de-listed, drop MaxLTV to zero
            let checked_vault_max_ltv = if *whitelisted && base_params.credit_manager.whitelisted {
                match self.kind {
                    AccountKind::Default => *max_loan_to_value,
                    AccountKind::HighLeveredStrategy => {
                        hls.as_ref().ok_or(MissingHLSParams(addr.to_string()))?.max_loan_to_value
                    }
                }
            } else {
                Decimal::zero()
            };

            max_ltv_adjusted_collateral = values
                .vault_coin
                .value
                .checked_mul_floor(checked_vault_max_ltv)?
                .checked_add(max_ltv_adjusted_collateral)?;

            let checked_liquidation_threshold = match self.kind {
                AccountKind::Default => *liquidation_threshold,
                AccountKind::HighLeveredStrategy => {
                    hls.as_ref().ok_or(MissingHLSParams(addr.to_string()))?.liquidation_threshold
                }
            };

            liquidation_threshold_adjusted_collateral = values
                .vault_coin
                .value
                .checked_mul_floor(checked_liquidation_threshold)?
                .checked_add(liquidation_threshold_adjusted_collateral)?;

            // Step 2: Calculate Base coin values
            let res = self.coins_value(&[Coin {
                denom: values.base_coin.denom.clone(),
                amount: v.amount.unlocking().total(),
            }])?;
            total_collateral_value =
                total_collateral_value.checked_add(res.total_collateral_value)?;
            max_ltv_adjusted_collateral =
                max_ltv_adjusted_collateral.checked_add(res.max_ltv_adjusted_collateral)?;
            liquidation_threshold_adjusted_collateral =
                liquidation_threshold_adjusted_collateral
                    .checked_add(res.liquidation_threshold_adjusted_collateral)?;
        }

        Ok(CollateralValue {
            total_collateral_value,
            max_ltv_adjusted_collateral,
            liquidation_threshold_adjusted_collateral,
        })
    }

    /// Total value of all spot debts.
    ///
    /// Denominated in the protocol's base asset (typically USDC).
    fn spot_debt_value(&self) -> HealthResult<Uint128> {
        let mut total = Uint128::zero();

        // spot debt borrowed from redbank
        for debt in &self.positions.debts {
            let coin_price =
                self.oracle_prices.get(&debt.denom).ok_or(MissingPrice(debt.denom.clone()))?;
            let debt_value = debt.amount.checked_mul_ceil(*coin_price)?;
            total = total.checked_add(debt_value)?;
        }

        Ok(total)
    }

    fn get_liquidation_ltv(&self, denom: &str) -> HealthResult<Decimal> {
        let AssetParams {
            liquidation_threshold,
            ..
        } = self.asset_params.get(denom).ok_or(MissingAssetParams(denom.to_string()))?;

        Ok(*liquidation_threshold)
    }

    fn get_perp_max_ltv(&self, denom: &str) -> HealthResult<Decimal> {
        let params =
            self.perps_data.params.get(denom).ok_or(MissingPerpParams(denom.to_string()))?;
        let denom_state =
            self.perps_data.denom_states.get(denom).ok_or(MissingDenomState(denom.to_string()))?;

        if !denom_state.enabled {
            return Ok(Decimal::zero());
        }

        Ok(params.max_loan_to_value)
    }

    fn get_perp_liq_ltv(&self, denom: &str) -> HealthResult<Decimal> {
        let params =
            self.perps_data.params.get(denom).ok_or(MissingPerpParams(denom.to_string()))?;

        let denom_state =
            self.perps_data.denom_states.get(denom).ok_or(MissingDenomState(denom.to_string()))?;

        if !denom_state.enabled {
            return Ok(Decimal::zero());
        }

        Ok(params.liquidation_threshold)
    }

    fn get_coin_max_ltv(&self, denom: &str) -> HealthResult<Decimal> {
        let params = self.asset_params.get(denom).ok_or(MissingAssetParams(denom.to_string()))?;

        // If the coin has been de-listed, drop MaxLTV to zero
        if !params.credit_manager.whitelisted {
            return Ok(Decimal::zero());
        }

        match self.kind {
            AccountKind::Default => Ok(params.max_loan_to_value),
            AccountKind::HighLeveredStrategy => Ok(params
                .credit_manager
                .hls
                .as_ref()
                .ok_or(MissingHLSParams(denom.to_string()))?
                .max_loan_to_value),
        }
    }

    fn get_coin_from_deposits_and_lends(&self, denom: &str) -> HealthResult<Coin> {
        let deposited_coin = self.positions.deposits.iter().find(|c| c.denom == denom);
        let deposited_amount = deposited_coin.unwrap_or(&Coin::default()).amount;

        let lent_coin = self.positions.lends.iter().find(|c| c.denom == denom);
        let lent_amount = lent_coin.unwrap_or(&Coin::default()).amount;

        Ok(Coin {
            denom: denom.to_string(),
            amount: deposited_amount.checked_add(lent_amount)?,
        })
    }

    // TODO - use comparison function
    fn get_min_and_max_funding_amounts(
        &self,
        position: &PerpPosition,
    ) -> HealthResult<(SignedDecimal, SignedDecimal)> {
        let accrued_funding_amount = position.unrealised_pnl.amounts.accrued_funding;
        // funding_max = max(0, unrealised_funding_accrued)
        let funding_max = if accrued_funding_amount.is_positive() {
            accrued_funding_amount
        } else {
            SignedDecimal::zero()
        };

        // funding min = -min(0, unrealised_funding_accrued)
        let funding_min = if accrued_funding_amount.is_negative() {
            accrued_funding_amount.abs.into()
        } else {
            SignedDecimal::zero()
        };

        Ok((funding_min, funding_max))
    }

    pub fn liquidation_price(
        &self,
        denom: &str,
        kind: &LiquidationPriceKind,
    ) -> HealthResult<Uint128> {
        let collateral_ltv_value = self.total_collateral_value()?.max_ltv_adjusted_collateral;
        let total_debt_value = self.spot_debt_value()?; // TODO: add perp debt value
        if total_debt_value.is_zero() {
            return Ok(Uint128::zero());
        }

        let current_price = self.oracle_prices.get(denom).ok_or(MissingPrice(denom.to_string()))?;

        if total_debt_value >= collateral_ltv_value {
            return Ok(Uint128::one() * *current_price);
        }

        match kind {
            LiquidationPriceKind::Asset => {
                let asset_amount = self.get_coin_from_deposits_and_lends(denom)?.amount;
                if asset_amount.is_zero() {
                    return Err(MissingAmount(denom.to_string()));
                }

                let asset_ltv = self.get_coin_max_ltv(denom)?;

                let asset_ltv_value =
                    asset_amount.checked_mul_floor(current_price.checked_mul(asset_ltv)?)?;
                let debt_with_asset_ltv_value = total_debt_value.checked_add(asset_ltv_value)?;

                if debt_with_asset_ltv_value <= collateral_ltv_value {
                    return Ok(Uint128::zero());
                }

                let debt_without = debt_with_asset_ltv_value - collateral_ltv_value;

                // liquidation_price = (debt_value - collateral_ltv_value + asset_ltv_value) / (asset_amount * asset_ltv)
                Ok(Uint128::one()
                    * Decimal::checked_from_ratio(debt_without, asset_amount)?.checked_mul(
                        Decimal::from_ratio(asset_ltv.denominator(), asset_ltv.numerator()),
                    )?)
            }

            LiquidationPriceKind::Debt => {
                let debt_amount = self
                    .positions
                    .debts
                    .iter()
                    .find(|c| c.denom == denom)
                    .ok_or(MissingAmount(denom.to_string()))?
                    .amount;
                if debt_amount.is_zero() {
                    return Err(MissingAmount(denom.to_string()));
                }

                // Liquidation_price = (collateral_ltv_value - total_debt_value + debt_value_asset / asset_amount
                let debt_value = debt_amount.checked_mul_ceil(*current_price)?;
                let net_collateral_value_without_debt =
                    collateral_ltv_value.checked_add(debt_value)?.checked_sub(total_debt_value)?;

                Ok(net_collateral_value_without_debt / debt_amount)
            }
        }
    }
}
