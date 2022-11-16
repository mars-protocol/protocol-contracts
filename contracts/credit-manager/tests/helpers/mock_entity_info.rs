use crate::helpers::CoinInfo;
use crate::helpers::VaultTestInfo;
use cosmwasm_std::{coin, Decimal};
use cw_utils::Duration;

pub fn uosmo_info() -> CoinInfo {
    CoinInfo {
        denom: "uosmo".to_string(),
        price: Decimal::from_atomics(25u128, 2).unwrap(),
        max_ltv: Decimal::from_atomics(7u128, 1).unwrap(),
        liquidation_threshold: Decimal::from_atomics(78u128, 2).unwrap(),
        liquidation_bonus: Decimal::from_atomics(12u128, 2).unwrap(),
    }
}
pub fn uatom_info() -> CoinInfo {
    CoinInfo {
        denom: "uatom".to_string(),
        price: Decimal::from_atomics(10u128, 1).unwrap(),
        max_ltv: Decimal::from_atomics(82u128, 2).unwrap(),
        liquidation_threshold: Decimal::from_atomics(9u128, 1).unwrap(),
        liquidation_bonus: Decimal::from_atomics(10u128, 2).unwrap(),
    }
}

pub fn ujake_info() -> CoinInfo {
    CoinInfo {
        denom: "ujake".to_string(),
        price: Decimal::from_atomics(23654u128, 4).unwrap(),
        max_ltv: Decimal::from_atomics(5u128, 1).unwrap(),
        liquidation_threshold: Decimal::from_atomics(55u128, 2).unwrap(),
        liquidation_bonus: Decimal::from_atomics(15u128, 2).unwrap(),
    }
}

pub fn lp_token_info() -> CoinInfo {
    CoinInfo {
        denom: "ugamm22".to_string(),
        price: Decimal::from_atomics(9874u128, 3).unwrap(),
        max_ltv: Decimal::from_atomics(63u128, 2).unwrap(),
        liquidation_threshold: Decimal::from_atomics(68u128, 2).unwrap(),
        liquidation_bonus: Decimal::from_atomics(12u128, 2).unwrap(),
    }
}

pub fn locked_vault_info() -> VaultTestInfo {
    generate_mock_vault(Some(Duration::Time(1_209_600))) // 14 days)
}

pub fn unlocked_vault_info() -> VaultTestInfo {
    generate_mock_vault(None)
}

pub fn generate_mock_vault(lockup: Option<Duration>) -> VaultTestInfo {
    let lp_token = lp_token_info();
    VaultTestInfo {
        vault_token_denom: "uleverage".to_string(),
        lockup,
        base_token_denom: lp_token.denom,
        deposit_cap: coin(10_000_000, "uusdc"),
        max_ltv: Decimal::from_atomics(6u128, 1).unwrap(),
        liquidation_threshold: Decimal::from_atomics(7u128, 1).unwrap(),
    }
}
