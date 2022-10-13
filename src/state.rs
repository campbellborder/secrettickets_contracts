use cosmwasm_std::{StdResult, StdError, CanonicalAddr, Storage};
use cosmwasm_storage::{
    Singleton, singleton, ReadonlySingleton, singleton_read, 
    PrefixedStorage, ReadonlyPrefixedStorage
};

use serde::{Serialize, Deserialize};

pub const KEY_CONFIG: &[u8] = b"config";
pub const PREFIX_BALANCES: &[u8] = b"balances";
pub const PREFIX_EVENTS: &[u8] = b"events";
pub const PREFIX_TICKETS: &[u8] = b"tickets";


#[derive(Serialize, Deserialize)]
pub struct Config {
    pub owner: CanonicalAddr
}

impl Config {
    pub fn new(owner: CanonicalAddr) -> Self {
        Self {
            owner: owner
        }
    }
}

pub fn get_config(storage: &mut dyn Storage) -> Singleton<Config> {
    singleton(storage, KEY_CONFIG)
}

pub fn get_config_readonly(storage: &dyn Storage) -> ReadonlySingleton<Config> {
    singleton_read(storage, KEY_CONFIG)
}


pub struct ReadonlyBalances<'a> {
    storage: ReadonlyPrefixedStorage<'a>
}

impl<'a> ReadonlyBalances<'a> {
    pub fn from_storage(storage: &'a mut dyn Storage) -> Self {
        Self {
            storage: ReadonlyPrefixedStorage::new(storage, PREFIX_BALANCES)
        }
    }

    pub fn read_account_balance(& mut self, account: &CanonicalAddr) -> u128 {
        let account_bytes = account.as_slice();
        let result = self.storage.get(account_bytes);
        match result {
            Some(balance_bytes) => slice_to_u128(&balance_bytes).unwrap(),
            None => 0,
        }
    }
}

pub struct Balances<'a> {
    storage: PrefixedStorage<'a>,
}

impl<'a> Balances<'a> {
    pub fn from_storage(storage: &'a mut dyn Storage) -> Self {
        Self {
            storage: PrefixedStorage::new(storage, PREFIX_BALANCES),
        }
    }

    pub fn set_account_balance(& mut self, account: &CanonicalAddr, amount: u128) {
        self.storage.set(account.as_slice(), &amount.to_be_bytes());
    }

    pub fn read_account_balance(& mut self, account: &CanonicalAddr) -> u128 {
        let account_bytes = account.as_slice();
        let result = self.storage.get(account_bytes);
        match result {
            Some(balance_bytes) => slice_to_u128(&balance_bytes).unwrap(),
            None => 0,
        }
    }
}

fn slice_to_u128(data: &[u8]) -> StdResult<u128> {
    match <[u8; 16]>::try_from(data) {
        Ok(bytes) => Ok(u128::from_be_bytes(bytes)),
        Err(_) => Err(StdError::generic_err(
            "Corrupted data found. 16 byte expected.",
        )),
    }
}