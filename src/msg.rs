use cosmwasm_std::{Addr, Uint128};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct InstantiateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Deposit {},
    Withdraw {amount: Uint128},
    CreateEvent {},
    BuyTicket {},
    VerifyTicket {},
    VerifyGuest {}
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    EventSoldOut {}
}

// Response for EventSoldOut query
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SoldOutResponse {
    pub sold_out: bool,
}

// Response for EventSoldOut query
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OwnerResponse {
    pub owner: Addr,
}