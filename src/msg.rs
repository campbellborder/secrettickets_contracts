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
    },
    BuyTicket {
        event_id: Uint128,
    },
    VerifyTicket {
        ticket_id: Uint128,
    },
    VerifyGuest {
        ticket_id: Uint128,
        secret: Uint128,
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
}

// Response for EventSoldOut query
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SoldOutResponse {
    pub sold_out: bool,
}

// Response for EventSoldOut query
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BalanceResponse {
    pub balance: Uint128,
}