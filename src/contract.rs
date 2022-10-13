use cosmwasm_std::{
    entry_point, to_binary, BankMsg, Coin, Deps, DepsMut, Env, MessageInfo, QueryResponse,
    Response, StdError, StdResult, Uint128,
};

use crate::msg::{ExecuteMsg, InstantiateMsg, OwnerResponse, QueryMsg, SoldOutResponse};
use crate::state::{get_config, get_config_readonly, Balances, Config};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> StdResult<Response> {
    // Construct contract config
    let owner_addr_canon = deps.api.addr_canonicalize(info.sender.as_str());
    let config = Config::new(owner_addr_canon.unwrap()); // Can we call unwrap safely here?

    // Save config
    get_config(deps.storage).save(&config)?;

    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, StdError> {
    match msg {
        ExecuteMsg::Deposit {} => try_deposit(deps, info),
        ExecuteMsg::Withdraw { amount } => try_withdraw(deps, info, amount),
        ExecuteMsg::CreateEvent {} => try_create_event(),
        ExecuteMsg::BuyTicket {} => try_buy_ticket(),
        ExecuteMsg::VerifyTicket {} => try_verify_ticket(),
        ExecuteMsg::VerifyGuest {} => try_verify_guest(),
    }
}

#[entry_point]
pub fn query(_deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<QueryResponse> {
    match msg {
        QueryMsg::EventSoldOut {} => to_binary(&query_event_sold_out()?),
    }
}

// Function to handle user depositing SCRT tokens for sEVNT tokens
pub fn try_deposit(deps: DepsMut, info: MessageInfo) -> Result<Response, StdError> {
    // Check if valid denomination tokens sent
    let mut amount = Uint128::zero();
    for coin in info.funds {
        if coin.denom == "uscrt" {
            amount = coin.amount;
        } else {
            return Err(StdError::generic_err(
                "Tried to deposit an unsupported token",
            ));
        }
    }

    // Check if non-negative number of tokens sent
    if amount.is_zero() {
        return Err(StdError::generic_err("No funds were sent to be deposited"));
    }

    // Get amount and address
    let raw_amount = amount.u128();
    let sender_address = deps.api.addr_canonicalize(info.sender.as_str())?;

    // Update balance
    let mut balances = Balances::from_storage(deps.storage);
    let account_balance = balances.read_account_balance(&sender_address);
    balances.set_account_balance(&sender_address, account_balance + raw_amount);

    // Success
    return Ok(Response::default());
}

// Function to handle user withdrawing sEVNT tokens for SCRT
pub fn try_withdraw(
    deps: DepsMut,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, StdError> {
    // Get sender address and amount to withdraw
    let sender_address = deps.api.addr_canonicalize(info.sender.as_str()).unwrap();
    let amount_raw = amount.u128();

    // Get current balance
    let mut balances = Balances::from_storage(deps.storage);
    let account_balance = balances.read_account_balance(&sender_address);
    // If enough available funds, update balance
    if account_balance >= amount_raw {
        balances.set_account_balance(&sender_address, account_balance - amount_raw);
    } else {
        return Err(StdError::generic_err(format!(
            "Insufficient funds to withdraw: balance={}, required={}",
            account_balance, amount_raw
        )));
    }

    // Get coins to withdraw
    let withdrawal_coins: Vec<Coin> = vec![Coin {
        denom: "uscrt".to_string(),
        amount,
    }];

    // Create and send response
    let response = Response::new().add_message(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: withdrawal_coins,
    });
    Ok(response)
}

pub fn try_create_event() -> Result<Response, StdError> {
    Ok(Response::default())
}

pub fn try_buy_ticket() -> Result<Response, StdError> {
    Ok(Response::default())
}

pub fn try_verify_ticket() -> Result<Response, StdError> {
    Ok(Response::default())
}

pub fn try_verify_guest() -> Result<Response, StdError> {
    Ok(Response::default())
}

fn _query_owner(deps: Deps) -> StdResult<OwnerResponse> {
    let config = get_config_readonly(deps.storage).load()?;
    let resp = OwnerResponse {
        owner: deps.api.addr_humanize(&config.owner).unwrap(),
    };

    Ok(resp)
}

fn query_event_sold_out() -> StdResult<SoldOutResponse> {
    let resp = SoldOutResponse { sold_out: true };

    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::state::ReadonlyBalances;
    use cosmwasm_std::coins;
    use cosmwasm_std::testing::{
        mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage,
    };
    use cosmwasm_std::{Addr, Api, Empty, OwnedDeps};

    fn instantiate_test() -> (
        Addr,
        OwnedDeps<MockStorage, MockApi, MockQuerier, Empty>,
        MessageInfo,
        InstantiateMsg,
    ) {
        let mut deps = mock_dependencies();

        let owner = deps.api.addr_validate("campbell").unwrap();
        let info = mock_info(owner.as_str(), &coins(1000, "earth"));
        let msg = InstantiateMsg {};

        let res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
        assert_eq!(0, res.messages.len());

        return (owner, deps, info, msg);
    }

    #[test]
    fn instantiate_proper() {
        let (owner, deps, _, _) = instantiate_test();

        // Check if owner is correct
        let owner_resp = _query_owner(deps.as_ref()).unwrap();
        assert_eq!(owner_resp.owner, owner);
    }

    #[test]
    fn deposit_proper() {

        // Instantiate contract
        let (owner, mut deps, _, _) = instantiate_test();

        // Deposit token
        let deposit_info = mock_info(owner.as_str(), &coins(1000, "uscrt"));
        let _deposit_resp = try_deposit(deps.as_mut(), deposit_info).unwrap();

        // Check if balance increased
        let owner_canon = deps.api.addr_canonicalize(owner.as_str()).unwrap();
        let mut balances = ReadonlyBalances::from_storage(deps.as_mut().storage);
        let owner_balance = balances.read_account_balance(&owner_canon);
        assert_eq!(owner_balance, 1000);
    }

    #[test]
    fn withdraw_proper() {
        // Instantiate contract
        let (owner, mut deps, _, _) = instantiate_test();

        // Deposit token
        let deposit_info = mock_info(owner.as_str(), &coins(1000, "uscrt"));
        let _deposit_resp = try_deposit(deps.as_mut(), deposit_info).unwrap();

        // Withdraw token
        let deposit_info = mock_info(owner.as_str(), &coins(0, "uscrt"));
        let _deposit_resp = try_withdraw(deps.as_mut(), deposit_info, Uint128::from(500u128)).unwrap();

        // Check if balance increased
        let owner_canon = deps.api.addr_canonicalize(owner.as_str()).unwrap();
        let mut balances = ReadonlyBalances::from_storage(deps.as_mut().storage);
        let owner_balance = balances.read_account_balance(&owner_canon);
        assert_eq!(owner_balance, 500);
    }

    #[test]
    fn deposit_invalid_token() {
        // Instantiate contract
        let (owner, mut deps, _, _) = instantiate_test();
        // Deposit token
        let deposit_info = mock_info(owner.as_str(), &coins(1000, "earth"));
        let deposit_resp = try_deposit(deps.as_mut(), deposit_info);

        // Should be error
        assert_eq!(deposit_resp.is_err(), true);
    }

    #[test]
    fn deposit_no_funds() {
        // Instantiate contract
        let (owner, mut deps, _, _) = instantiate_test();
        // Deposit token
        let deposit_info = mock_info(owner.as_str(), &coins(0, "uscrt"));
        let deposit_resp = try_deposit(deps.as_mut(), deposit_info);

        // Should be error
        assert_eq!(deposit_resp.is_err(), true);
    }

    #[test]
    fn withdraw_not_enough_funds() {
        // Instantiate contract
        let (owner, mut deps, _, _) = instantiate_test();

        // Deposit token
        let deposit_info = mock_info(owner.as_str(), &coins(1000, "uscrt"));
        let _deposit_resp = try_deposit(deps.as_mut(), deposit_info).unwrap();

        // Withdraw token
        let deposit_info = mock_info(owner.as_str(), &coins(0, "uscrt"));
        let deposit_resp = try_withdraw(deps.as_mut(), deposit_info, Uint128::from(1500u128));

        // Should be error
        assert_eq!(deposit_resp.is_err(), true);
    }
}
