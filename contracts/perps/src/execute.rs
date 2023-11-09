use cosmwasm_std::{
    coin, coins, to_binary, Addr, BankMsg, Coin, Decimal, DepsMut, Env, MessageInfo, Response,
    StdError, Storage, Uint128, WasmMsg,
};
use cw_utils::{may_pay, must_pay, nonpayable};
use mars_types::{
    credit_manager::{self, Action},
    math::SignedDecimal,
    oracle::ActionKind,
    perps::{Config, DenomState, Funding, PnL, Position, UnlockState, VaultState},
};

use crate::{
    error::{ContractError, ContractResult},
    pnl::{compute_pnl, DenomStateExt},
    state::{
        decrease_deposit_shares, increase_deposit_shares, CONFIG, DENOM_STATES, OWNER, POSITIONS,
        UNLOCKS, VAULT_STATE,
    },
    vault::{amount_to_shares, shares_to_amount},
};

pub fn initialize(store: &mut dyn Storage, cfg: Config<Addr>) -> ContractResult<Response> {
    CONFIG.save(store, &cfg)?;

    // initialize vault state to zero total liquidity and zero total shares
    VAULT_STATE.save(store, &VaultState::default())?;

    Ok(Response::new().add_attribute("method", "initialize"))
}

pub fn init_denom(
    store: &mut dyn Storage,
    env: Env,
    sender: &Addr,
    denom: &str,
    max_funding_velocity: Decimal,
    skew_scale: Decimal,
) -> ContractResult<Response> {
    OWNER.assert_owner(store, sender)?;

    if DENOM_STATES.has(store, denom) {
        return Err(ContractError::DenomAlreadyExists {
            denom: denom.into(),
        });
    }

    if skew_scale.is_zero() {
        return Err(ContractError::InvalidParam {
            reason: "skew_scale cannot be zero".to_string(),
        });
    }

    let denom_state = DenomState {
        enabled: true,
        total_size: SignedDecimal::zero(),
        total_cost_base: SignedDecimal::zero(),
        funding: Funding {
            max_funding_velocity,
            skew_scale,
            constant_factor: Funding::constant_factor(max_funding_velocity, skew_scale)?,
            rate: SignedDecimal::zero(),
            index: SignedDecimal::one(),
            accumulated_size_weighted_by_index: SignedDecimal::zero(),
        },
        last_updated: env.block.time.seconds(),
    };
    DENOM_STATES.save(store, denom, &denom_state)?;

    Ok(Response::new()
        .add_attribute("method", "init_denom")
        .add_attribute("denom", denom)
        .add_attribute("max_funding_velocity", max_funding_velocity.to_string())
        .add_attribute("skew_scale", skew_scale.to_string()))
}

pub fn enable_denom(
    store: &mut dyn Storage,
    env: Env,
    sender: &Addr,
    denom: &str,
) -> ContractResult<Response> {
    OWNER.assert_owner(store, sender)?;

    DENOM_STATES.update(store, denom, |maybe_ds| {
        // if the denom does not already exist then we cannot enable it
        let Some(mut ds) = maybe_ds else {
            return Err(ContractError::DenomNotFound {
                denom: denom.into(),
            });
        };

        // if the denom already exists, if must have not already been enabled
        if ds.enabled {
            return Err(ContractError::DenomEnabled {
                denom: denom.into(),
            });
        }

        // now we know the denom exists and is not enabled
        // flip the enabled parameter to true and return
        ds.enabled = true;

        // When denom is disabled there is no trading activity so funding shouldn't be changed.
        // We just shift the last_updated time.
        ds.last_updated = env.block.time.seconds();

        Ok(ds)
    })?;

    Ok(Response::new().add_attribute("method", "enable_denom").add_attribute("denom", denom))
}

pub fn disable_denom(
    store: &mut dyn Storage,
    env: Env,
    sender: &Addr,
    denom: &str,
) -> ContractResult<Response> {
    OWNER.assert_owner(store, sender)?;

    DENOM_STATES.update(store, denom, |maybe_ds| {
        let Some(mut ds) = maybe_ds else {
            return Err(ContractError::DenomNotFound {
                denom: denom.into(),
            });
        };

        let current_time = env.block.time.seconds();

        // refresh funding rate and index before disabling trading
        let current_funding = ds.compute_current_funding(current_time)?;
        ds.funding = current_funding;

        ds.enabled = false;
        ds.last_updated = current_time;

        Ok(ds)
    })?;

    Ok(Response::new().add_attribute("method", "disable_denom").add_attribute("denom", denom))
}

pub fn deposit(store: &mut dyn Storage, info: MessageInfo) -> ContractResult<Response> {
    let cfg = CONFIG.load(store)?;
    let mut vs = VAULT_STATE.load(store)?;

    // find the deposit amount
    let amount = must_pay(&info, &cfg.base_denom)?;

    // compute the new shares to be minted to the depositor
    let shares = amount_to_shares(&vs, amount)?;

    // increment total liquidity and deposit shares
    vs.total_liquidity = vs.total_liquidity.checked_add(amount)?;
    vs.total_shares = vs.total_shares.checked_add(shares)?;
    VAULT_STATE.save(store, &vs)?;

    // increment the user's deposit shares
    increase_deposit_shares(store, &info.sender, shares)?;

    Ok(Response::new()
        .add_attribute("method", "deposit")
        .add_attribute("amount", amount)
        .add_attribute("shares", shares))
}

pub fn unlock(
    store: &mut dyn Storage,
    current_time: u64,
    depositor: &Addr,
    shares: Uint128,
) -> ContractResult<Response> {
    let cfg = CONFIG.load(store)?;
    let mut vs = VAULT_STATE.load(store)?;

    // convert the shares to amount
    let amount = shares_to_amount(&vs, shares)?;

    // cannot unlock when there is zero shares
    if amount.is_zero() {
        return Err(ContractError::ZeroShares);
    }

    // decrement total liquidity and deposit shares
    vs.total_liquidity = vs.total_liquidity.checked_sub(amount)?;
    vs.total_shares = vs.total_shares.checked_sub(shares)?;
    VAULT_STATE.save(store, &vs)?;

    // decrement the user's deposit shares
    decrease_deposit_shares(store, depositor, shares)?;

    // add new unlock position
    let cooldown_end = current_time + cfg.cooldown_period;
    UNLOCKS.update(store, depositor, |maybe_unlocks| {
        let mut unlocks = maybe_unlocks.unwrap_or_default();

        unlocks.push(UnlockState {
            created_at: current_time,
            cooldown_end,
            amount,
        });

        Ok::<Vec<UnlockState>, StdError>(unlocks)
    })?;

    Ok(Response::new()
        .add_attribute("method", "unlock")
        .add_attribute("amount", amount)
        .add_attribute("shares", shares)
        .add_attribute("created_at", current_time.to_string())
        .add_attribute("cooldown_end", cooldown_end.to_string()))
}

pub fn withdraw(
    store: &mut dyn Storage,
    current_time: u64,
    depositor: &Addr,
) -> ContractResult<Response> {
    let cfg = CONFIG.load(store)?;
    let unlocks = UNLOCKS.load(store, depositor)?;

    // find all unlocked positions
    let (unlocked, unlocking): (Vec<_>, Vec<_>) =
        unlocks.into_iter().partition(|us| us.cooldown_end <= current_time);

    // cannot withdraw when there is zero unlocked positions
    if unlocked.is_empty() {
        return Err(ContractError::UnlockedPositionsNotFound {});
    }

    // clear state if no more unlocking positions
    if unlocking.is_empty() {
        UNLOCKS.remove(store, depositor);
    } else {
        UNLOCKS.save(store, depositor, &unlocking)?;
    }

    // compute the total amount to be withdrawn
    let unlocked_amt = unlocked.into_iter().map(|us| us.amount).sum::<Uint128>();

    Ok(Response::new()
        .add_attribute("method", "withdraw")
        .add_attribute("amount", unlocked_amt)
        .add_message(BankMsg::Send {
            to_address: depositor.into(),
            amount: coins(unlocked_amt.u128(), cfg.base_denom),
        }))
}

pub fn open_position(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    account_id: String,
    denom: String,
    size: SignedDecimal,
) -> ContractResult<Response> {
    let cfg = CONFIG.load(deps.storage)?;

    // no payment is expected when opening a position
    nonpayable(&info)?;

    // query the asset's price
    //
    // this will be the position's entry price, used to compute PnL when closing
    // the position
    let entry_price = cfg.oracle.query_price(&deps.querier, &denom, ActionKind::Default)?.price;

    // only the credit manager contract can open positions
    if info.sender != cfg.credit_manager {
        return Err(ContractError::SenderIsNotCreditManager);
    }

    // the denom must exists and have been enabled
    let mut ds = DENOM_STATES.load(deps.storage, &denom)?;
    if !ds.enabled {
        return Err(ContractError::DenomNotEnabled {
            denom,
        });
    }

    // the position's initial value cannot be too small
    let value = size.abs.checked_mul(entry_price)?.to_uint_floor();
    if value < cfg.min_position_value {
        return Err(ContractError::PositionTooSmall {
            min: cfg.min_position_value,
            found: value,
        });
    }

    // each account can only have one position for a denom at the same time
    if POSITIONS.has(deps.storage, (&account_id, &denom)) {
        return Err(ContractError::PositionExists {
            account_id,
            denom,
        });
    }

    // Update the denom's accumulators.
    // Funding rates and index is updated to the current block time (using old size).
    ds.open_position(env.block.time.seconds(), size, entry_price)?;
    DENOM_STATES.save(deps.storage, &denom, &ds)?;

    // save the user's new position with updated funding index
    POSITIONS.save(
        deps.storage,
        (&account_id, &denom),
        &Position {
            size,
            entry_price,
            entry_funding_index: ds.funding.index,
        },
    )?;

    Ok(Response::new()
        .add_attribute("method", "open_position")
        .add_attribute("account_id", account_id)
        .add_attribute("denom", denom)
        .add_attribute("size", size.to_string())
        .add_attribute("entry_price", entry_price.to_string()))
}

pub fn close_position(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    account_id: String,
    denom: String,
) -> ContractResult<Response> {
    let mut res = Response::new();

    let cfg = CONFIG.load(deps.storage)?;
    let mut vs = VAULT_STATE.load(deps.storage)?;
    let mut ds = DENOM_STATES.load(deps.storage, &denom)?;
    let position = POSITIONS.load(deps.storage, (&account_id, &denom))?;

    // when closing a position, the credit manager may send no coin (in case the
    // the position is winning or breaking even) or one coin of the base denom
    // (in case the position is losing)
    let paid_amount = may_pay(&info, &cfg.base_denom)?;

    // only the credit manager contract can close positions
    if info.sender != cfg.credit_manager {
        return Err(ContractError::SenderIsNotCreditManager);
    }

    // query the current price of the asset
    let exit_price = cfg.oracle.query_price(&deps.querier, &denom, ActionKind::Default)?.price;

    // Update the denom's accumulators.
    // Funding rates and index is updated to the current block time (using old size).
    ds.close_position(env.block.time.seconds(), &position)?;

    // compute the position's unrealized PnL
    let pnl = compute_pnl(&ds.funding, &position, exit_price, &cfg.base_denom)?;

    // compute how many coins should be returned to the credit account, and
    // update global liquidity amount
    let refund_amount = match &pnl {
        PnL::Profit(Coin {
            amount,
            ..
        }) => {
            vs.total_liquidity = vs.total_liquidity.checked_sub(*amount)?;
            paid_amount.checked_add(*amount)?
        }
        PnL::Loss(Coin {
            amount,
            ..
        }) => {
            vs.total_liquidity = vs.total_liquidity.checked_add(*amount)?;
            paid_amount.checked_sub(*amount)?
        }
        PnL::BreakEven => paid_amount,
    };

    if !refund_amount.is_zero() {
        res = res.add_message(WasmMsg::Execute {
            contract_addr: cfg.credit_manager.into(),
            msg: to_binary(&credit_manager::ExecuteMsg::UpdateCreditAccount {
                account_id: account_id.clone(),
                actions: vec![Action::Deposit(coin(refund_amount.u128(), &cfg.base_denom))],
            })?,
            funds: coins(refund_amount.u128(), cfg.base_denom),
        });
    }

    // save the updated global state and delete position
    VAULT_STATE.save(deps.storage, &vs)?;
    POSITIONS.remove(deps.storage, (&account_id, &denom));
    DENOM_STATES.save(deps.storage, &denom, &ds)?;

    Ok(res
        .add_attribute("method", "close_position")
        .add_attribute("account_id", account_id)
        .add_attribute("denom", denom)
        .add_attribute("size", position.size.to_string())
        .add_attribute("entry_price", position.entry_price.to_string())
        .add_attribute("exit_price", exit_price.to_string())
        .add_attribute("realized_pnl", pnl.to_string()))
}
