use crate::state::{ALLOWED_ASSETS, ALLOWED_VAULTS, CREDIT_ACCOUNT_NFT_CONTRACT, OWNER};
use cosmwasm_std::{Addr, Deps, Order, StdResult};
use cw_asset::{AssetInfo, AssetInfoKey, AssetInfoUnchecked};
use cw_storage_plus::Bound;
use std::convert::TryFrom;

const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

pub fn query_nft_contract_addr(deps: Deps) -> StdResult<String> {
    Ok(CREDIT_ACCOUNT_NFT_CONTRACT.load(deps.storage)?.into())
}

pub fn query_owner(deps: Deps) -> StdResult<String> {
    Ok(OWNER.load(deps.storage)?.into())
}

/// NOTE: This implementation of the query function assumes the map `ALLOWED_VAULTS` only saves `true`.
/// If a vault is to be removed from the whitelist, the map must remove the correspoinding key, instead
/// of setting the value to `false`.
pub fn query_allowed_vaults(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<Vec<String>> {
    let addr: Addr;
    let start = match &start_after {
        Some(addr_str) => {
            addr = deps.api.addr_validate(addr_str)?;
            Some(Bound::exclusive(addr))
        }
        None => None,
    };

    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

    ALLOWED_VAULTS
        .keys(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|res| res.map(|vault_addr| vault_addr.to_string()))
        .collect()
}

/// NOTE: This implementation of the query function assumes the map `ALLOWED_ASSETS` only saves `true`.
/// If an asset is to be removed from the whitelist, the map must remove the corresponding key, instead
/// of setting the value to `false`.
pub fn query_allowed_assets(
    deps: Deps,
    start_after: Option<AssetInfoUnchecked>,
    limit: Option<u32>,
) -> StdResult<Vec<AssetInfoUnchecked>> {
    let info: AssetInfo;
    let start = match &start_after {
        Some(unchecked) => {
            info = unchecked.check(deps.api, None)?;
            Some(Bound::exclusive(AssetInfoKey::from(info)))
        }
        None => None,
    };

    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

    ALLOWED_ASSETS
        .keys(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .collect::<StdResult<Vec<_>>>()?
        .into_iter()
        .map(|key| AssetInfoUnchecked::try_from(key))
        .collect()
}
