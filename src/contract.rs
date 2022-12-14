use cosmwasm_std::{
    entry_point, to_binary, Addr, BankMsg, Coin, Deps, DepsMut, Env, MessageInfo, QueryResponse,
    Response, StdError, StdResult, Uint128,
};

use hex;

use rsa::{PublicKey, RsaPublicKey, pkcs8::DecodePublicKey, PaddingScheme};
use rand::{SeedableRng};
use rand_chacha::ChaChaRng;

use crate::msg::{
    BalanceResponse, EventsResponse, ExecuteMsg, InstantiateMsg, QueryMsg, SoldOutResponse,
    TicketsResponse,
};
use crate::state::{
    get_config, Balances, Config, Event, Events, GuestsTickets, OrganisersEvents, ReadonlyBalances,
    ReadonlyEvents, ReadonlyGuestsTickets, ReadonlyOrganisersEvents, ReadonlyTickets, Ticket,
    Tickets,
};

use extprim::u128;

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
        ExecuteMsg::CreateEvent { price, max_tickets, entropy } => {
            try_create_event(deps, info, price, max_tickets, entropy)
        }
        ExecuteMsg::BuyTicket { event_id, entropy, pk } => try_buy_ticket(deps, info, event_id, entropy, pk),
        ExecuteMsg::VerifyTicket { ticket_id } => try_verify_ticket(deps, info, ticket_id),
        ExecuteMsg::VerifyGuest { ticket_id, secret } => {
            try_verify_guest(deps, info, ticket_id, secret)
        }
    }
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<QueryResponse> {
    match msg {
        QueryMsg::EventSoldOut { event_id } => to_binary(&query_event_sold_out(deps, event_id)?),
        QueryMsg::Balance { address } => to_binary(&query_balance(deps, address)?),
        QueryMsg::Events { address } => to_binary(&query_events(deps, address)?),
        QueryMsg::Tickets { address } => to_binary(&query_tickets(deps, address)?),
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
    entropy: String
) -> Result<Response, StdError> {
    // Get raw inputs and organiser address
    let price_raw = price.u128();
    let max_tickets_raw = max_tickets.u128();
    let entropy_raw = match u128::from_str_radix(&entropy, 16) {
        Result::Ok(number) => number,
        Result::Err(_) => {
            return Err(StdError::generic_err(format!("Entropy is not a valid 32 byte hex string",)));
        }
    };
    let organiser = deps.api.addr_canonicalize(info.sender.as_str()).unwrap();

    // Get next event ID
    let mut config = get_config(deps.storage).load()?;
    let event_id = config.get_next_event_id();
    get_config(deps.storage).save(&config)?;

    // Create event
    let event = Event::new(event_id, organiser.clone(), price_raw, max_tickets_raw, entropy_raw);

    // Store event in events
    let mut events = Events::from_storage(deps.storage);
    events.store_event(event_id, &event);

    // Store event in organisers events
    let mut organisers_events = OrganisersEvents::from_storage(deps.storage);
    let mut this_organisers_events = organisers_events.load_events(&organiser);
    this_organisers_events.push(event_id);
    organisers_events.store_events(&organiser, &this_organisers_events);

    // Respond with eventID
    let response = Response::new().add_attribute("event_id", event_id.to_string());
    Ok(response)
}

pub fn try_buy_ticket(
    deps: DepsMut,
    info: MessageInfo,
    event_id: Uint128,
    entropy: String,
    pk: String
) -> Result<Response, StdError> {
    // Get raw inputs and guest address
    let event_id_raw = event_id.u128();
    let entropy_raw = match u128::from_str_radix(&entropy, 16) {
        Result::Ok(number) => number,
        Result::Err(_) => {
            return Err(StdError::generic_err(format!("Entropy is not a valid 32 byte hex string",)));
        }
    };

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

    // Ensure guest does not already own a ticket to this event
    let guests_tickets = GuestsTickets::from_storage(deps.storage);
    let this_guests_tickets = guests_tickets.load_tickets(&guest);
    let tickets = Tickets::from_storage(deps.storage);
    for ticket_id in this_guests_tickets {
        let ticket = tickets.may_load_ticket(ticket_id).unwrap();
        if ticket.get_event_id() == event_id_raw {
            return Err(StdError::generic_err(format!("You already own a ticket to this event",)));
        }
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

    // Transfer funds
    balances.set_account_balance(&guest, guest_balance - event_price);
    let organiser_balance = balances.read_account_balance(event.get_organiser());
    balances.set_account_balance(event.get_organiser(), organiser_balance + event_price);

    // Record ticket sale in event
    event.ticket_sold(entropy_raw);
    let mut events = Events::from_storage(deps.storage);
    events.store_event(event.get_id(), &event);

    // Get next ticket id
    let mut config = get_config(deps.storage).load()?;
    let ticket_id = config.get_next_ticket_id();
    get_config(deps.storage).save(&config)?;

    // Create ticket
    let secret = event.generate_secret(u128::u128::from_built_in(ticket_id));
    let ticket = Ticket::new(ticket_id, event_id_raw, guest.clone(), secret, pk);

    // Store ticket in tickets
    let mut tickets = Tickets::from_storage(deps.storage);
    tickets.store_ticket(ticket_id, &ticket);

    // Store event in guests tickets
    let mut guests_tickets = GuestsTickets::from_storage(deps.storage);
    let mut this_guests_tickets = guests_tickets.load_tickets(&guest);
    this_guests_tickets.push(ticket_id);
    guests_tickets.store_tickets(&guest, &this_guests_tickets);

    // Respond with ticketID
    let response = Response::new().add_attribute("ticket_id", ticket_id.to_string());
    Ok(response)
}

pub fn try_verify_ticket(
    deps: DepsMut,
    info: MessageInfo,
    ticket_id: Uint128,
) -> Result<Response, StdError> {
    // Get raw inputs and 'organiser' address
    let ticket_id_raw = ticket_id.u128();
    let organiser = deps.api.addr_canonicalize(info.sender.as_str()).unwrap();

    // Ensure ticket exists and load it
    let tickets = ReadonlyTickets::from_storage(deps.storage);
    let mut ticket = match tickets.may_load_ticket(ticket_id_raw) {
        Some(ticket) => ticket.clone(),
        None => {
            return Err(StdError::generic_err(format!("Ticket does not exist")));
        }
    };

    // Ensure ticket is not used
    if ticket.get_state() == 2 {
        return Err(StdError::generic_err(format!(
            "Ticket has already been used"
        )));
    }

    // Check message sender is organiser of event
    let events = ReadonlyEvents::from_storage(deps.storage);
    let event = events.may_load_event(ticket.get_event_id()).unwrap();
    if *event.get_organiser() != organiser {
        return Err(StdError::generic_err(format!(
            "You are not the organiser of this event"
        )));
    }

    // Generate secret and set ticket status to validating
    let secret = ticket.start_validation();
    let pk = ticket.get_pk();
    let mut tickets = Tickets::from_storage(deps.storage);
    tickets.store_ticket(ticket_id_raw, &ticket);

    // Encrypt with public key of guest
    let mut rng = ChaChaRng::from_seed(event.get_seed());
    let public_key = RsaPublicKey::from_public_key_pem(&pk).unwrap();
    let padding = PaddingScheme::new_pkcs1v15_encrypt();
    let secret_encrypted = public_key.encrypt(&mut rng, padding, &secret.to_be_bytes()).unwrap();

    // Respond with encrypted secret
    let response = Response::new().add_attribute("secret_encrypted", hex::encode(secret_encrypted));
    Ok(response)
}

pub fn try_verify_guest(
    deps: DepsMut,
    info: MessageInfo,
    ticket_id: Uint128,
    secret: String,
) -> Result<Response, StdError> {
    // Get raw inputs and 'organiser' address
    let ticket_id_raw = ticket_id.u128();
    let secret_raw = match u64::from_str_radix(&secret, 16) {
        Result::Ok(number) => number,
        Result::Err(_) => {
            return Err(StdError::generic_err(format!("Secret is not a valid 16 byte hex string",)));
        }
    };
    let organiser = deps.api.addr_canonicalize(info.sender.as_str()).unwrap();

    // Ensure ticket exists and load it
    let tickets = ReadonlyTickets::from_storage(deps.storage);
    let mut ticket = match tickets.may_load_ticket(ticket_id_raw) {
        Some(ticket) => ticket.clone(),
        None => {
            return Err(StdError::generic_err(format!("Ticket does not exist")));
        }
    };

    // Ensure ticket is in validating state
    match ticket.get_state() {
        0 => {
            return Err(StdError::generic_err(format!(
                "Validation of ticket not initiated yet"
            )))
        }
        1 => (),
        2 => {
            return Err(StdError::generic_err(format!(
                "Ticket has already been used"
            )))
        }
        _ => {
            return Err(StdError::generic_err(format!(
                "Ticket is somehow in invalid state"
            )))
        }
    };

    // Check message sender is organiser of event
    let events = ReadonlyEvents::from_storage(deps.storage);
    let event = events.may_load_event(ticket.get_event_id()).unwrap();
    if *event.get_organiser() != organiser {
        return Err(StdError::generic_err(format!(
            "You are not the organiser of this event"
        )));
    }

    // Check if secret is correct
    match ticket.try_verify(secret_raw) {
        Ok(()) => {
            let mut tickets = Tickets::from_storage(deps.storage);
            tickets.store_ticket(ticket_id_raw, &ticket);
            Ok(Response::default())
        }
        Err(err) => Err(err),
    }
}

fn query_event_sold_out(deps: Deps, event_id: Uint128) -> StdResult<SoldOutResponse> {
    let event_id_raw = event_id.u128();
    let events = ReadonlyEvents::from_storage(deps.storage);
    match events.may_load_event(event_id_raw) {
        Some(event) => Ok(SoldOutResponse {
            sold_out: event.is_sold_out(),
        }),
        None => Err(StdError::generic_err(format!("Event does not exist",))),
    }
}

fn query_balance(deps: Deps, address: Addr) -> StdResult<BalanceResponse> {
    let address_canon = deps.api.addr_canonicalize(address.as_str())?;
    let balances = ReadonlyBalances::from_storage(deps.storage);
    Ok(BalanceResponse {
        balance: Uint128::from(balances.read_account_balance(&address_canon)),
    })
}

fn query_events(deps: Deps, address: Addr) -> StdResult<EventsResponse> {
    let address_canon = deps.api.addr_canonicalize(address.as_str())?;
    let organisers_events = ReadonlyOrganisersEvents::from_storage(deps.storage);
    let this_organisers_events = organisers_events.load_events(&address_canon);
    let events = ReadonlyEvents::from_storage(deps.storage);

    let mut events_vec = vec![];
    let mut tickets_vec = vec![];
    for event_id in this_organisers_events {

        let event = events.may_load_event(event_id).unwrap();
        events_vec.push(Uint128::from(event_id));
        tickets_vec.push(Uint128::from(event.get_tickets_left()));
    }
    Ok(EventsResponse { events: events_vec, tickets_left: tickets_vec })
}

fn query_tickets(deps: Deps, address: Addr) -> StdResult<TicketsResponse> {
    let address_canon = deps.api.addr_canonicalize(address.as_str())?;
    let guests_tickets = ReadonlyGuestsTickets::from_storage(deps.storage);
    let this_guests_tickets = guests_tickets.load_tickets(&address_canon);
    let tickets = ReadonlyTickets::from_storage(deps.storage);

    let mut tickets_vec = vec![];
    let mut events_vec = vec![];
    let mut state_vec: Vec<Uint128> = vec![];
    for ticket_id in this_guests_tickets {

        // Load ticket
        let ticket = tickets.may_load_ticket(ticket_id).unwrap();

        // Create return vectors
        tickets_vec.push(Uint128::from(ticket_id));
        events_vec.push(Uint128::from(ticket.get_event_id()));
        state_vec.push(Uint128::from(ticket.get_state()));
    }
    Ok(TicketsResponse {
        tickets: tickets_vec,
        events: events_vec,
        states: state_vec,
    })
}

#[cfg(test)]
mod tests {

    use super::*;

    use crate::state::{get_config_readonly, ReadonlyBalances};
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
        let config = get_config_readonly(&deps.storage).load().unwrap();
        assert_eq!(deps.api.addr_humanize(config.get_owner()).unwrap(), owner);
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
        let owner_canon = deps.api.addr_canonicalize(owner.as_str()).unwrap();

        // Create event
        let price = Uint128::from(500u128);
        let max_tickets = Uint128::from(500u128);
        let info = mock_info(owner.as_str(), &coins(0, "uscrt"));
        let entropy = "986192837319283719".to_string();
        let mut resp = try_create_event(deps.as_mut(), info, price, max_tickets, entropy).unwrap();

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
        assert_eq!(
            deps.api.addr_humanize(event.get_organiser()).unwrap(),
            owner
        );

        // Check in organisers events
        let organisers_events = ReadonlyOrganisersEvents::from_storage(deps.as_mut().storage);
        let this_organisers_events = organisers_events.load_events(&owner_canon);
        assert_eq!(*this_organisers_events.get(0).unwrap(), event_id);

        // Create event
        let entropy = "12761237192837192".to_string();
        let info = mock_info(owner.as_str(), &coins(0, "uscrt"));
        let mut resp = try_create_event(deps.as_mut(), info, price, max_tickets, entropy).unwrap();

        // Check proper event ID emitted
        let attribute = resp.attributes.pop().unwrap();
        assert_eq!(attribute.key, "event_id");
        assert_eq!(attribute.value, "2");

        let organisers_events = ReadonlyOrganisersEvents::from_storage(deps.as_mut().storage);
        let this_organisers_events = organisers_events.load_events(&owner_canon);
        assert_eq!(*this_organisers_events.get(1).unwrap(), 2);
    }

    // #[test]
    // fn buy_ticket_proper() {
    //     // Instantiate contract
    //     let (owner, mut deps, _, _) = instantiate_test();

    //     // Deposit tokens
    //     let guest = deps.api.addr_validate("guest").unwrap();
    //     let deposit_info = mock_info(guest.as_str(), &coins(1000, "uscrt"));
    //     let _deposit_resp = try_deposit(deps.as_mut(), deposit_info).unwrap();

    //     // Create event
    //     let price = Uint128::from(50u128);
    //     let max_tickets = Uint128::from(500u128);
    //     let info = mock_info(owner.as_str(), &coins(0, "uscrt"));
    //     let entropy = Uint128::from(3457263458762u128);
    //     let mut resp = try_create_event(deps.as_mut(), info, price, max_tickets, entropy).unwrap();
    //     let attribute = resp.attributes.pop().unwrap();
    //     let event_id: u128 = attribute.value.parse().unwrap();

    //     // Buy ticket
    //     let entropy = Uint128::from(1827391824732872934872u128);
    //     let info = mock_info(guest.as_str(), &coins(0, "uscrt"));
    //     let mut resp = try_buy_ticket(deps.as_mut(), info, Uint128::from(event_id), entropy).unwrap();

    //     // Check proper ticket ID emitted
    //     let attribute = resp.attributes.pop().unwrap();
    //     assert_eq!(attribute.key, "ticket_id");
    //     assert_eq!(attribute.value, "1");

    //     // Check ticket in storage
    //     let ticket_id: u128 = attribute.value.parse().unwrap();
    //     assert_eq!(ticket_id, 1);
    //     let tickets = ReadonlyTickets::from_storage(deps.as_mut().storage);
    //     let ticket = tickets.may_load_ticket(ticket_id).unwrap();
    //     assert_eq!(ticket.get_id(), ticket_id);
    //     assert_eq!(ticket.get_event_id(), event_id);
    //     assert_eq!(deps.api.addr_humanize(ticket.get_guest()).unwrap(), guest);

    //     // Check event ticket count incremented
    //     let events = ReadonlyEvents::from_storage(deps.as_mut().storage);
    //     let event = events.may_load_event(event_id).unwrap();
    //     assert_eq!(event.get_tickets_sold(), 1);

    //     // Check guest balance decreased
    //     let guest_address = deps.api.addr_canonicalize(guest.as_str()).unwrap();
    //     let balances = ReadonlyBalances::from_storage(deps.as_mut().storage);
    //     let guest_balance = balances.read_account_balance(&guest_address);
    //     assert_eq!(guest_balance, 950);

    //     // Check organiser balance decreased
    //     let organiser_address = deps.api.addr_canonicalize(owner.as_str()).unwrap();
    //     let balances = ReadonlyBalances::from_storage(deps.as_mut().storage);
    //     let organiser_balance = balances.read_account_balance(&organiser_address);
    //     assert_eq!(organiser_balance, 50);
    // }

    // #[test]
    // fn verify_ticket_proper() {
    //     // Instantiate contract
    //     let (owner, mut deps, _, _) = instantiate_test();

    //     // Deposit tokens
    //     let guest = deps.api.addr_validate("guest").unwrap();
    //     let deposit_info = mock_info(guest.as_str(), &coins(1000, "uscrt"));
    //     let _deposit_resp = try_deposit(deps.as_mut(), deposit_info).unwrap();

    //     // Create event
    //     let price = Uint128::from(50u128);
    //     let max_tickets = Uint128::from(500u128);
    //     let info = mock_info(owner.as_str(), &coins(0, "uscrt"));
    //     let entropy = Uint128::from(3457263458762u128);
    //     let mut resp = try_create_event(deps.as_mut(), info, price, max_tickets, entropy).unwrap();
    //     let attribute = resp.attributes.pop().unwrap();
    //     let event_id: u128 = attribute.value.parse().unwrap();

    //     // Buy ticket
    //     let entropy = Uint128::from(1827391824732872934872u128);
    //     let info = mock_info(guest.as_str(), &coins(0, "uscrt"));
    //     let mut resp = try_buy_ticket(deps.as_mut(), info, Uint128::from(event_id), entropy).unwrap();

    //     // Get ticket
    //     let attribute = resp.attributes.pop().unwrap();
    //     let ticket_id: u128 = attribute.value.parse().unwrap();

    //     // Begin to verify ticket and get secret
    //     let info = mock_info(owner.as_str(), &coins(0, "uscrt"));
    //     let mut resp = try_verify_ticket(deps.as_mut(), info, Uint128::from(ticket_id)).unwrap();
    //     let attribute = resp.attributes.pop().unwrap();
    //     assert_eq!(attribute.key, "secret_encrypted");
    //     assert_eq!(attribute.value, "9662036190035425912");
    //     let _secret_encrypted: u128 = attribute.value.parse().unwrap();

    //     // Check ticket is in validating state
    //     let tickets = ReadonlyTickets::from_storage(deps.as_mut().storage);
    //     let ticket = tickets.may_load_ticket(ticket_id).unwrap();
    //     assert_eq!(ticket.get_state(), 1);

    //     // // Validate guest
    //     // let info = mock_info(owner.as_str(), &coins(0, "uscrt"));
    //     // try_verify_guest(
    //     //     deps.as_mut(),
    //     //     info,
    //     //     Uint128::from(ticket_id),
    //     //     Uint128::from(9662036190035425912u128.div_euclid(2)),
    //     // )
    //     // .unwrap();

    //     // // Check ticket is in used state
    //     // let tickets = ReadonlyTickets::from_storage(deps.as_mut().storage);
    //     // let ticket = tickets.may_load_ticket(ticket_id).unwrap();
    //     // assert_eq!(ticket.get_state(), 2);
    // }

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

    #[test]
    fn random_shit() {
        let num: u64 = 69;
        let bytes = num.to_be_bytes();
        assert_eq!(bytes, vec![1, 2].as_slice())
    }

}
