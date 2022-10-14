use cosmwasm_std::{
    entry_point, to_binary, BankMsg, Coin, Deps, DepsMut, Env, MessageInfo, QueryResponse,
    Response, StdError, StdResult, Uint128,
};

use crate::msg::{ExecuteMsg, InstantiateMsg, OwnerResponse, QueryMsg, SoldOutResponse};
use crate::state::{
    Config, get_config, get_config_readonly,
    Balances, 
    Event, Events, ReadonlyEvents,
    Ticket, Tickets
};

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
        ExecuteMsg::CreateEvent { price, max_tickets } => {
            try_create_event(deps, info, price, max_tickets)
        }
        ExecuteMsg::BuyTicket { event_id } => try_buy_ticket(deps, info, event_id),
        ExecuteMsg::VerifyTicket { ticket_id } => try_verify_ticket(deps, info, ticket_id),
        ExecuteMsg::VerifyGuest { secret } => try_verify_guest(deps, info, secret),
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

pub fn try_create_event(
    deps: DepsMut,
    info: MessageInfo,
    price: Uint128,
    max_tickets: Uint128,
) -> Result<Response, StdError> {
    // Get raw inputs and organiser address
    let price_raw = price.u128();
    let max_tickets_raw = max_tickets.u128();
    let organiser_address = deps.api.addr_canonicalize(info.sender.as_str()).unwrap();

    // Get next event ID
    let mut config = get_config(deps.storage).load()?;
    let event_id = config.get_next_event_id();

    // Create event
    let event = Event::new(event_id, organiser_address, price_raw, max_tickets_raw);

    // Store event
    let mut events = Events::from_storage(deps.storage);
    events.store_event(event_id, &event);

    // Respond with eventID
    let response = Response::new().add_attribute("event_id", event_id.to_string());
    Ok(response)
}

pub fn try_buy_ticket(
    deps: DepsMut,
    info: MessageInfo,
    event_id: Uint128,
) -> Result<Response, StdError> {
    // Get raw inputs and guest address
    let event_id_raw = event_id.u128();
    let guest = deps.api.addr_canonicalize(info.sender.as_str()).unwrap();

    // Ensure event exists and is not sold out
    let events = ReadonlyEvents::from_storage(deps.storage);
    let mut event = match events.may_load_event(event_id_raw) {
        Some(event) => event.clone(),
        None => {
            return Err(StdError::generic_err(format!("Event does not exist",)));
        }
    };
    if event.is_sold_out() {
        return Err(StdError::generic_err(format!("Event is sold out",)));
    }

    // Ensure guest has sufficient funds
    let mut balances = Balances::from_storage(deps.storage);
    let guest_balance = balances.read_account_balance(&guest);
    let event_price = event.get_price();
    if guest_balance < event_price {
        return Err(StdError::generic_err(format!(
            "Insufficient funds: balance={}, required={}",
            guest_balance, event_price,
        )));
    }

    // Withdraw funds
    balances.set_account_balance(&guest, guest_balance - event_price);

    // Record ticket sale in event
    event.ticket_sold();
    let mut events = Events::from_storage(deps.storage);
    events.store_event(event.get_id(), &event);

    // Get next ticket id
    let mut config = get_config(deps.storage).load()?;
    let ticket_id = config.get_next_ticket_id();

    // Create ticket
    let ticket = Ticket::new(ticket_id, event_id_raw, guest);

    // Store ticket
    let mut tickets = Tickets::from_storage(deps.storage);
    tickets.store_ticket(ticket_id, &ticket);

    // Respond with ticketID
    let response = Response::new().add_attribute("ticket_id", ticket_id.to_string());
    Ok(response)
}

pub fn try_verify_ticket(
    _deps: DepsMut,
    _info: MessageInfo,
    _ticket_id: Uint128,
) -> Result<Response, StdError> {
    Ok(Response::default())
}

pub fn try_verify_guest(
    _deps: DepsMut,
    _info: MessageInfo,
    _secret: Uint128,
) -> Result<Response, StdError> {
    Ok(Response::default())
}

fn _query_owner(deps: Deps) -> StdResult<OwnerResponse> {
    let config = get_config_readonly(deps.storage).load()?;
    let resp = OwnerResponse {
        owner: deps.api.addr_humanize(config.get_owner()).unwrap(),
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

    use crate::state::{ReadonlyBalances, ReadonlyTickets};
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

        let owner = deps.api.addr_validate("owner").unwrap();
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

        // Deposit tokens
        let deposit_info = mock_info(owner.as_str(), &coins(1000, "uscrt"));
        let _deposit_resp = try_deposit(deps.as_mut(), deposit_info).unwrap();

        // Check if balance increased
        let owner_canon = deps.api.addr_canonicalize(owner.as_str()).unwrap();
        let balances = ReadonlyBalances::from_storage(deps.as_mut().storage);
        let owner_balance = balances.read_account_balance(&owner_canon);
        assert_eq!(owner_balance, 1000);
    }

    #[test]
    fn withdraw_proper() {
        // Instantiate contract
        let (owner, mut deps, _, _) = instantiate_test();

        // Deposit tokens
        let deposit_info = mock_info(owner.as_str(), &coins(1000, "uscrt"));
        let _deposit_resp = try_deposit(deps.as_mut(), deposit_info).unwrap();

        // Withdraw tokens
        let deposit_info = mock_info(owner.as_str(), &coins(0, "uscrt"));
        let _deposit_resp =
            try_withdraw(deps.as_mut(), deposit_info, Uint128::from(500u128)).unwrap();

        // Check if balance increased
        let owner_canon = deps.api.addr_canonicalize(owner.as_str()).unwrap();
        let balances = ReadonlyBalances::from_storage(deps.as_mut().storage);
        let owner_balance = balances.read_account_balance(&owner_canon);
        assert_eq!(owner_balance, 500);
    }

    #[test]
    fn create_event_proper() {
        // Instantiate contract
        let (owner, mut deps, _, _) = instantiate_test();

        // Create event
        let price = Uint128::from(500u128);
        let max_tickets = Uint128::from(500u128);
        let info = mock_info(owner.as_str(), &coins(0, "uscrt"));
        let mut resp = try_create_event(deps.as_mut(), info, price, max_tickets).unwrap();
        
        // Check proper event ID emitted
        let attribute = resp.attributes.pop().unwrap();
        assert_eq!(attribute.key, "event_id");
        assert_eq!(attribute.value, "1");

        // Check in storage
        let event_id: u128 = attribute.value.parse().unwrap();
        assert_eq!(event_id, 1);
        let events = ReadonlyEvents::from_storage(deps.as_mut().storage);
        let event = events.may_load_event(event_id).unwrap();

        assert_eq!(event.get_id(), event_id);
        assert_eq!(event.get_price(), price.u128());
        assert_eq!(event.get_max_tickets(), max_tickets.u128());
        assert_eq!(event.get_tickets_sold(), 0);
        assert_eq!(deps.api.addr_humanize(event.get_organiser()).unwrap(), owner);
    }

    #[test]
    fn buy_ticket_proper() {
        // Instantiate contract
        let (owner, mut deps, _, _) = instantiate_test();

        // Deposit tokens
        let guest = deps.api.addr_validate("guest").unwrap();
        let deposit_info = mock_info(guest.as_str(), &coins(1000, "uscrt"));
        let _deposit_resp = try_deposit(deps.as_mut(), deposit_info).unwrap();

        // Create event
        let price = Uint128::from(50u128);
        let max_tickets = Uint128::from(500u128);
        let info = mock_info(owner.as_str(), &coins(0, "uscrt"));
        let mut resp = try_create_event(deps.as_mut(), info, price, max_tickets).unwrap();
        let attribute = resp.attributes.pop().unwrap();
        let event_id: u128 = attribute.value.parse().unwrap();

        // Buy ticket
        let info = mock_info(guest.as_str(), &coins(0, "uscrt"));
        let mut resp = try_buy_ticket(deps.as_mut(), info, Uint128::from(event_id)).unwrap();
        
        // Check proper ticket ID emitted
        let attribute = resp.attributes.pop().unwrap();
        assert_eq!(attribute.key, "ticket_id");
        assert_eq!(attribute.value, "1");

        // Check ticket in storage
        let ticket_id: u128 = attribute.value.parse().unwrap();
        assert_eq!(ticket_id, 1);
        let tickets = ReadonlyTickets::from_storage(deps.as_mut().storage);
        let ticket = tickets.may_load_ticket(ticket_id).unwrap();
        assert_eq!(ticket.get_id(), ticket_id);
        assert_eq!(ticket.get_event_id(), event_id);
        assert_eq!(deps.api.addr_humanize(ticket.get_guest()).unwrap(), guest);

        // Check event ticket count incremented
        let events = ReadonlyEvents::from_storage(deps.as_mut().storage);
        let event = events.may_load_event(event_id).unwrap();
        assert_eq!(event.get_tickets_sold(), 1);

        // Check guest balance decreased
        let guest_address = deps.api.addr_canonicalize(guest.as_str()).unwrap();
        let balances = ReadonlyBalances::from_storage(deps.as_mut().storage);
        let guest_balance = balances.read_account_balance(&guest_address);
        assert_eq!(guest_balance, 950);

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
