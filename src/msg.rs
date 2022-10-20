use cosmwasm_std::{Uint128, Addr};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct InstantiateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Deposit {},
    Withdraw {
        amount: Uint128,
    },
    CreateEvent {
        price: Uint128,
        max_tickets: Uint128,
        entropy: String
    },
    BuyTicket {
        event_id: Uint128,
        entropy: String,
        pk: String
    },
    VerifyTicket {
        ticket_id: Uint128,
    },
    VerifyGuest {
        ticket_id: Uint128,
        secret: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Balance {
        address: Addr
    },
    EventSoldOut {
        event_id: Uint128
    },
    Events {
        address: Addr
    },
    Tickets {
        address: Addr
    }
}

// Response for EventSoldOut query
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SoldOutResponse {
    pub sold_out: bool,
}

// Response for Balance query
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BalanceResponse {
    pub balance: Uint128,
}

// Response for Events query
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EventsResponse {
    pub events: Vec<Uint128>,
    pub tickets_left: Vec<Uint128>,
}

// Response for Tickets query
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TicketsResponse {
    pub tickets: Vec<Uint128>,
    pub events: Vec<Uint128>,
    pub states: Vec<Uint128>
}