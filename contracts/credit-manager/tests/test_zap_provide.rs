use cosmwasm_std::OverflowOperation::Sub;
use cosmwasm_std::{Addr, OverflowError, Uint128};
use mars_mock_zapper::contract::STARTING_LP_POOL_TOKENS;

use mars_mock_zapper::error::ContractError;
use mars_rover::error::ContractError as RoverError;
use mars_rover::msg::execute::Action::{Deposit, ProvideLiquidity};

use crate::helpers::{
    assert_err, get_coin, lp_token_info, uatom_info, ujake_info, uosmo_info, AccountToFund, MockEnv,
};

pub mod helpers;

#[test]
fn test_only_token_owner_can_zap_for_account() {
    let user = Addr::unchecked("user");
    let mut mock = MockEnv::new().build().unwrap();
    let account_id = mock.create_credit_account(&user).unwrap();

    let another_user = Addr::unchecked("another_user");
    let res = mock.update_credit_account(
        &account_id,
        &another_user,
        vec![ProvideLiquidity {
            coins_in: vec![],
            lp_token_out: "".to_string(),
            minimum_receive: Default::default(),
        }],
        &[],
    );

    assert_err(
        res,
        RoverError::NotTokenOwner {
            user: another_user.clone().into(),
            account_id: account_id.clone(),
        },
    );
}

#[test]
fn test_does_not_have_enough_tokens_to_provide_liq() {
    let atom = uatom_info();
    let osmo = uosmo_info();
    let lp_token = lp_token_info();

    let user = Addr::unchecked("user");
    let mut mock = MockEnv::new()
        .allowed_coins(&[lp_token.clone(), atom.clone(), osmo.clone()])
        .fund_account(AccountToFund {
            addr: user.clone(),
            funds: vec![atom.to_coin(300), osmo.to_coin(300)],
        })
        .build()
        .unwrap();

    let account_id = mock.create_credit_account(&user).unwrap();
    let res = mock.update_credit_account(
        &account_id,
        &user,
        vec![
            Deposit(atom.to_coin(100)),
            Deposit(osmo.to_coin(50)),
            ProvideLiquidity {
                coins_in: vec![atom.to_coin(100), osmo.to_coin(200)],
                lp_token_out: lp_token.denom,
                minimum_receive: Uint128::zero(),
            },
        ],
        &[atom.to_coin(100), osmo.to_coin(50)],
    );

    assert_err(
        res,
        RoverError::Overflow(OverflowError {
            operation: Sub,
            operand1: "50".to_string(),
            operand2: "200".to_string(),
        }),
    )
}

#[test]
fn test_lp_token_out_must_be_whitelisted() {
    let atom = uatom_info();
    let osmo = uosmo_info();
    let lp_token = lp_token_info();

    let user = Addr::unchecked("user");
    let mut mock = MockEnv::new()
        .allowed_coins(&[atom.clone(), osmo.clone()])
        .fund_account(AccountToFund {
            addr: user.clone(),
            funds: vec![atom.to_coin(300), osmo.to_coin(300)],
        })
        .build()
        .unwrap();

    let account_id = mock.create_credit_account(&user).unwrap();
    let res = mock.update_credit_account(
        &account_id,
        &user,
        vec![
            Deposit(atom.to_coin(100)),
            Deposit(osmo.to_coin(50)),
            ProvideLiquidity {
                coins_in: vec![atom.to_coin(100), osmo.to_coin(200)],
                lp_token_out: lp_token.denom.clone(),
                minimum_receive: Uint128::zero(),
            },
        ],
        &[atom.to_coin(100), osmo.to_coin(50)],
    );

    assert_err(res, RoverError::NotWhitelisted(lp_token.denom))
}

#[test]
fn test_coins_in_must_be_whitelisted() {
    let atom = uatom_info();
    let osmo = uosmo_info();
    let lp_token = lp_token_info();

    let user = Addr::unchecked("user");
    let mut mock = MockEnv::new()
        .allowed_coins(&[lp_token.clone(), osmo.clone()])
        .fund_account(AccountToFund {
            addr: user.clone(),
            funds: vec![atom.to_coin(300), osmo.to_coin(300)],
        })
        .build()
        .unwrap();

    let account_id = mock.create_credit_account(&user).unwrap();
    let res = mock.update_credit_account(
        &account_id,
        &user,
        vec![ProvideLiquidity {
            coins_in: vec![atom.to_coin(100), osmo.to_coin(200)],
            lp_token_out: lp_token.denom,
            minimum_receive: Uint128::zero(),
        }],
        &[],
    );

    assert_err(res, RoverError::NotWhitelisted(atom.denom))
}

#[test]
fn test_min_received_too_high() {
    let atom = uatom_info();
    let osmo = uosmo_info();
    let lp_token = lp_token_info();

    let user = Addr::unchecked("user");
    let mut mock = MockEnv::new()
        .allowed_coins(&[lp_token.clone(), atom.clone(), osmo.clone()])
        .fund_account(AccountToFund {
            addr: user.clone(),
            funds: vec![atom.to_coin(300), osmo.to_coin(300)],
        })
        .build()
        .unwrap();

    let account_id = mock.create_credit_account(&user).unwrap();
    let err = mock
        .update_credit_account(
            &account_id,
            &user,
            vec![
                Deposit(atom.to_coin(100)),
                Deposit(osmo.to_coin(50)),
                ProvideLiquidity {
                    coins_in: vec![atom.to_coin(100), osmo.to_coin(50)],
                    lp_token_out: lp_token.denom,
                    minimum_receive: Uint128::new(100_000_000_000),
                },
            ],
            &[atom.to_coin(100), osmo.to_coin(50)],
        )
        .unwrap_err();

    let contract_err: ContractError = err.downcast().unwrap();
    assert_eq!(contract_err, ContractError::ReceivedBelowMinimum);
}

#[test]
fn test_wrong_denom_provided() {
    let atom = uatom_info();
    let jake = ujake_info();
    let lp_token = lp_token_info();

    let user = Addr::unchecked("user");
    let mut mock = MockEnv::new()
        .allowed_coins(&[lp_token.clone(), atom.clone(), jake.clone()])
        .fund_account(AccountToFund {
            addr: user.clone(),
            funds: vec![atom.to_coin(300), jake.to_coin(300)],
        })
        .build()
        .unwrap();

    let account_id = mock.create_credit_account(&user).unwrap();
    let err = mock
        .update_credit_account(
            &account_id,
            &user,
            vec![
                Deposit(atom.to_coin(100)),
                Deposit(jake.to_coin(50)),
                ProvideLiquidity {
                    coins_in: vec![atom.to_coin(100), jake.to_coin(50)],
                    lp_token_out: lp_token.denom.clone(),
                    minimum_receive: Uint128::zero(),
                },
            ],
            &[atom.to_coin(100), jake.to_coin(50)],
        )
        .unwrap_err();

    let contract_err: ContractError = err.downcast().unwrap();
    assert_eq!(
        contract_err,
        ContractError::RequirementsNotMet {
            lp_token: lp_token.denom,
            coin0: atom.denom,
            coin1: "uosmo".to_string()
        }
    );
}

#[test]
fn test_successful_zap() {
    let atom = uatom_info();
    let osmo = uosmo_info();
    let lp_token = lp_token_info();

    let user = Addr::unchecked("user");
    let mut mock = MockEnv::new()
        .allowed_coins(&[lp_token.clone(), atom.clone(), osmo.clone()])
        .fund_account(AccountToFund {
            addr: user.clone(),
            funds: vec![atom.to_coin(300), osmo.to_coin(300)],
        })
        .build()
        .unwrap();

    let account_id = mock.create_credit_account(&user).unwrap();
    let estimate =
        mock.estimate_provide_liquidity(&lp_token.denom, &[atom.to_coin(100), osmo.to_coin(50)]);
    let slippage_adjusted = estimate.multiply_ratio(Uint128::new(95), Uint128::new(100));
    assert_eq!(slippage_adjusted, Uint128::new(950_000)); // 1_000_000 * .95

    mock.update_credit_account(
        &account_id,
        &user,
        vec![
            Deposit(atom.to_coin(100)),
            Deposit(osmo.to_coin(50)),
            ProvideLiquidity {
                coins_in: vec![atom.to_coin(100), osmo.to_coin(50)],
                lp_token_out: lp_token.denom.clone(),
                minimum_receive: slippage_adjusted,
            },
        ],
        &[atom.to_coin(100), osmo.to_coin(50)],
    )
    .unwrap();

    // assert follow up estimate can be made (calculates ratio from first deposit)
    let estimate =
        mock.estimate_provide_liquidity(&lp_token.denom, &[atom.to_coin(300), osmo.to_coin(150)]);
    assert_eq!(estimate, STARTING_LP_POOL_TOKENS * Uint128::new(3)); // 3x the size as first deposit

    // assert user's new position
    let positions = mock.query_positions(&account_id);
    assert_eq!(positions.coins.len(), 1);
    let lp_balance = get_coin(&lp_token.denom, &positions.coins);
    assert_eq!(lp_balance.amount, STARTING_LP_POOL_TOKENS);

    // assert rover actually has the tokens
    let lp_balance = mock.query_balance(&mock.rover, &lp_token.denom);
    assert_eq!(lp_balance.amount, STARTING_LP_POOL_TOKENS);
    let atom_balance = mock.query_balance(&mock.rover, &atom.denom);
    assert_eq!(atom_balance.amount, Uint128::zero());
    let osmo_balance = mock.query_balance(&mock.rover, &osmo.denom);
    assert_eq!(osmo_balance.amount, Uint128::zero());

    // assert coin balance of zapper contract
    let config = mock.query_config();
    let lp_balance = mock.query_balance(&Addr::unchecked(config.zapper.clone()), &lp_token.denom);
    // prefunded minus minted
    assert_eq!(
        lp_balance.amount,
        Uint128::new(10_000_000) - STARTING_LP_POOL_TOKENS
    );
    let atom_balance = mock.query_balance(&Addr::unchecked(config.zapper.clone()), &atom.denom);
    assert_eq!(atom_balance.amount, Uint128::new(100));
    let osmo_balance = mock.query_balance(&Addr::unchecked(config.zapper), &osmo.denom);
    assert_eq!(osmo_balance.amount, Uint128::new(50));
}
