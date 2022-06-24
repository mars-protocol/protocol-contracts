use cosmwasm_std::Addr;
use cw721::OwnerOfResponse;
use cw721_base::{ExecuteMsg, Extension, InstantiateMsg, MintMsg, QueryMsg};
use cw_multi_test::{App, AppResponse, BasicApp, ContractWrapper, Executor};
use std::fmt::Error;

use crate::contract::{execute, instantiate, query};

#[test]
fn test_id_incrementer() {
    let mut app = App::default();
    let contract = Box::new(ContractWrapper::new(execute, instantiate, query));
    let owner = Addr::unchecked("owner");
    let code_id = app.store_code(contract);
    let contract_addr = instantiate_mock_nft_contract(&mut app, &owner, code_id);

    let user_1 = Addr::unchecked("user_1");
    let res = mint_action(&mut app, &owner, &contract_addr, &user_1).unwrap();
    let token_id = get_token_id(res);
    assert_eq!(token_id, "1");
    assert_owner_is_correct(&mut app, &contract_addr, &user_1, &token_id);

    let user_2 = Addr::unchecked("user_2");
    let res = mint_action(&mut app, &owner, &contract_addr, &user_2).unwrap();
    let token_id = get_token_id(res);
    assert_eq!(token_id, "2");
    assert_owner_is_correct(&mut app, &contract_addr, &user_2, &token_id);

    let user_3 = Addr::unchecked("user_3");
    let res = mint_action(&mut app, &owner, &contract_addr, &user_3).unwrap();
    let token_id = get_token_id(res);
    assert_eq!(token_id, "3");
    assert_owner_is_correct(&mut app, &contract_addr, &user_3, &token_id);
}

#[test]
fn test_only_owner_can_mint() {
    let mut app = App::default();
    let contract = Box::new(ContractWrapper::new(execute, instantiate, query));
    let owner = Addr::unchecked("owner");
    let code_id = app.store_code(contract);
    let contract_addr = instantiate_mock_nft_contract(&mut app, &owner, code_id);

    let bad_guy = Addr::unchecked("bad_guy");
    let res = mint_action(&mut app, &bad_guy, &contract_addr, &bad_guy);
    match res {
        Ok(_) => panic!("Should have thrown an error"),
        Err(_) => {}
    }
}

// Double checking ownership by querying NFT account-nft for correct owner
fn assert_owner_is_correct(
    app: &mut BasicApp,
    contract_addr: &Addr,
    user: &Addr,
    token_id: &String,
) {
    let owner_res: OwnerOfResponse = app
        .wrap()
        .query_wasm_smart(
            contract_addr,
            &QueryMsg::OwnerOf {
                token_id: token_id.clone(),
                include_expired: None,
            },
        )
        .unwrap();

    assert_eq!(user.to_string(), owner_res.owner)
}

fn instantiate_mock_nft_contract(app: &mut BasicApp, owner: &Addr, code_id: u64) -> Addr {
    let contract_addr = app
        .instantiate_contract(
            code_id,
            owner.clone(),
            &InstantiateMsg {
                name: String::from("mock_nft"),
                symbol: String::from("MOCK"),
                minter: owner.to_string(),
            },
            &[],
            "mock-account-nft",
            None,
        )
        .unwrap();
    contract_addr
}

fn mint_action(
    app: &mut BasicApp,
    owner: &Addr,
    contract_addr: &Addr,
    user: &Addr,
) -> Result<AppResponse, Error> {
    app.execute_contract(
        owner.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Mint {
            0: MintMsg {
                token_id: String::from("some_token_id_that_will_be_ignored"),
                owner: user.to_string(),
                token_uri: None,
                extension: Extension::None,
            },
        },
        &[],
    )
    .map_err(|_| Error::default())
}

fn get_token_id(res: AppResponse) -> String {
    let attr: Vec<&String> = res
        .events
        .iter()
        .flat_map(|event| &event.attributes)
        .filter(|attr| attr.key == "token_id")
        .map(|attr| &attr.value)
        .collect();

    assert_eq!(attr.len(), 1);
    attr.first().unwrap().to_string()
}
