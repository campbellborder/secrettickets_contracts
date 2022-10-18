use cosmwasm_std::{StdResult, StdError, CanonicalAddr, Storage};
use cosmwasm_storage::{
    Singleton, singleton, ReadonlySingleton, singleton_read, 
    PrefixedStorage, ReadonlyPrefixedStorage
};

use serde::{Serialize, Deserialize};
use bincode;

// Storage keys
pub const KEY_CONFIG: &[u8] = b"config";
pub const PREFIX_BALANCES: &[u8] = b"balances";
pub const PREFIX_EVENTS: &[u8] = b"events";
pub const PREFIX_TICKETS: &[u8] = b"tickets";
pub const PREFIX_ORGANISERS_EVENTS: &[u8] = b"organisers_events";
pub const PREFIX_GUESTS_TICKETS: &[u8] = b"guests_tickets";

// Struct to store contract config
#[derive(Serialize, Deserialize)]
pub struct Config {
    owner: CanonicalAddr,
    num_events: u128,
    num_tickets: u128
}

impl Config {
    pub fn new(owner: CanonicalAddr) -> Self {
        Self {
            owner: owner,
            num_events: 0,
            num_tickets: 0
        }
    }

    pub fn get_owner(&self) -> &CanonicalAddr {
        &self.owner
    }

    pub fn get_num_events(&self) -> u128 {
        self.num_events
    }

    pub fn get_num_tickets(&self) -> u128 {
        self.num_tickets
    }

    pub fn get_next_event_id(&mut self) -> u128 {
        self.num_events += 1;
        self.num_events
    }

    pub fn get_next_ticket_id(&mut self) -> u128 {
        self.num_tickets += 1;
        self.num_tickets
    }

}

// Get config singleton storage structure
pub fn get_config(storage: &mut dyn Storage) -> Singleton<Config> {
    singleton(storage, KEY_CONFIG)
}

// Get READONLY config singleton storage struture
pub fn get_config_readonly(storage: &dyn Storage) -> ReadonlySingleton<Config> {
    singleton_read(storage, KEY_CONFIG)
}

// Struct to handle READONLY interaction with balances 
pub struct ReadonlyBalances<'a> {
    storage: ReadonlyPrefixedStorage<'a>
}

impl<'a> ReadonlyBalances<'a> {

    // Retrieve prefixed storage
    pub fn from_storage(storage: &'a dyn Storage) -> Self {
        Self {
            storage: ReadonlyPrefixedStorage::new(storage, PREFIX_BALANCES)
        }
    }

    // Read balance of an account
    pub fn read_account_balance(&self, account: &CanonicalAddr) -> u128 {
        let account_bytes = account.as_slice();
        let result = self.storage.get(account_bytes);
        match result {
            Some(balance_bytes) => slice_to_u128(&balance_bytes).unwrap(),
            None => 0,
        }
    }
}

// Struct to handle interaction with balances 
pub struct Balances<'a> {
    storage: PrefixedStorage<'a>,
}

impl<'a> Balances<'a> {

    // Retrieve prefixed storage
    pub fn from_storage(storage: &'a mut dyn Storage) -> Self {
        Self {
            storage: PrefixedStorage::new(storage, PREFIX_BALANCES),
        }
    }

    // Set balance of an account
    pub fn set_account_balance(& mut self, account: &CanonicalAddr, amount: u128) {
        self.storage.set(account.as_slice(), &amount.to_be_bytes());
    }

    // Read balance of an account
    pub fn read_account_balance(&self, account: &CanonicalAddr) -> u128 {
        let account_bytes = account.as_slice();
        let result = self.storage.get(account_bytes);
        match result {
            Some(balance_bytes) => slice_to_u128(&balance_bytes).unwrap(),
            None => 0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Event {
    id: u128,
    organiser: CanonicalAddr,
    price: u128,
    max_tickets: u128,
    tickets_sold: u128
}

impl Event {
    pub fn new(id: u128, organiser: CanonicalAddr, price: u128, max_tickets: u128) -> Self {
        Event {
            id: id,
            organiser: organiser,
            price: price,
            max_tickets: max_tickets,
            tickets_sold: 0
        }
    }

    pub fn get_id(&self) -> u128 {
        self.id
    }

    pub fn get_organiser(&self) -> &CanonicalAddr {
        &self.organiser
    }

    pub fn get_price(&self) -> u128 {
        self.price
    }

    pub fn get_max_tickets(&self) -> u128 {
        self.max_tickets
    }

    pub fn get_tickets_sold(&self) -> u128 {
        self.tickets_sold
    }

    pub fn is_sold_out(&self) -> bool {
        self.tickets_sold >= self.max_tickets
    }

    pub fn ticket_sold(& mut self) {
        self.tickets_sold += 1;
    }
}

// Struct to handle interaction with events
pub struct Events<'a> {
    storage: PrefixedStorage<'a>,
}

impl<'a> Events<'a> {

    // Retrieve prefixed storage
    pub fn from_storage(storage: &'a mut dyn Storage) -> Self {
        Self {
            storage: PrefixedStorage::new(storage, PREFIX_EVENTS),
        }
    }

    // Store event
    pub fn store_event(& mut self, event_id: u128, event: &Event) {
        self.storage.set(&event_id.to_be_bytes(), &bincode::serialize(event).unwrap());
    }

    // Try load an event
    pub fn may_load_event(&self, event_id: u128) -> Option<Event> {
        let id_bytes = event_id.to_be_bytes();
        match self.storage.get(&id_bytes) {
            Some(event_bytes) => Option::Some(bincode::deserialize(&event_bytes).unwrap()),
            None => None
        }
    }
}

// Struct to handle READONLY interaction with events 
pub struct ReadonlyEvents<'a> {
    storage: ReadonlyPrefixedStorage<'a>
}

impl<'a> ReadonlyEvents<'a> {

    // Retrieve prefixed storage
    pub fn from_storage(storage: &'a dyn Storage) -> Self {
        Self {
            storage: ReadonlyPrefixedStorage::new(storage, PREFIX_EVENTS)
        }
    }

    // Try load an event
    pub fn may_load_event(&self, event_id: u128) -> Option<Event> {
        let id_bytes = event_id.to_be_bytes();
        match self.storage.get(&id_bytes) {
            Some(event_bytes) => Option::Some(bincode::deserialize(&event_bytes).unwrap()),
            None => None
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Ticket {
    id: u128,
    guest: CanonicalAddr,
    event_id: u128,
    state: u8,
    secret: u128
}

impl Ticket {
    pub fn new(id: u128, event_id: u128, guest: CanonicalAddr) -> Self {
        Ticket {
            id: id, 
            event_id: event_id, 
            guest: guest,
            state: 0,
            secret: 0
        }
    }

    pub fn get_id(&self) -> u128 {
        self.id
    }
    
    pub fn get_event_id(&self) -> u128 {
        self.event_id
    }

    pub fn get_guest(&self) -> &CanonicalAddr {
        &self.guest
    }

    pub fn get_state(&self) -> u8 {
        self.state
    }

    pub fn start_validation(&mut self) -> u128 {
        self.state = 1;
        self.secret = 69;
        self.secret
    }

    pub fn try_verify(&mut self, secret: u128) -> StdResult<()> {
        if self.secret != secret {
            return Err(StdError::generic_err("Secret does not match"));
        }

        self.secret = 0;
        self.state = 2;
        Ok(())
    }
}

// Struct to handle interaction with tickets
pub struct Tickets<'a> {
    storage: PrefixedStorage<'a>,
}

impl<'a> Tickets<'a> {

    // Retrieve prefixed storage
    pub fn from_storage(storage: &'a mut dyn Storage) -> Self {
        Self {
            storage: PrefixedStorage::new(storage, PREFIX_TICKETS),
        }
    }

    // Store ticket
    pub fn store_ticket(& mut self, ticket_id: u128, ticket: &Ticket) {
        self.storage.set(&ticket_id.to_be_bytes(), &bincode::serialize(ticket).unwrap());
    }

    // Try load a ticket
    pub fn may_load_ticket(&self, ticket_id: u128) -> Option<Ticket> {
        let id_bytes = ticket_id.to_be_bytes();
        match self.storage.get(&id_bytes) {
            Some(ticket_bytes) => Option::Some(bincode::deserialize(&ticket_bytes).unwrap()),
            None => None
        }
    }

    // Delete a ticket?
}

// Struct to handle READONLY interaction with events 
pub struct ReadonlyTickets<'a> {
    storage: ReadonlyPrefixedStorage<'a>
}

impl<'a> ReadonlyTickets<'a> {

    // Retrieve prefixed storage
    pub fn from_storage(storage: &'a dyn Storage) -> Self {
        Self {
            storage: ReadonlyPrefixedStorage::new(storage, PREFIX_TICKETS)
        }
    }

    // Try load a ticket
    pub fn may_load_ticket(&self, ticket_id: u128) -> Option<Ticket> {
        let id_bytes = ticket_id.to_be_bytes();
        match self.storage.get(&id_bytes) {
            Some(ticket_bytes) => Option::Some(bincode::deserialize(&ticket_bytes).unwrap()),
            None => None
        }
    }
}

// Struct to handle interaction with organisers events
pub struct OrganisersEvents<'a> {
    storage: PrefixedStorage<'a>
}

impl<'a> OrganisersEvents<'a> {

    // Retrieve prefixed storage
    pub fn from_storage(storage: &'a mut dyn Storage) -> Self {
        Self {
            storage: PrefixedStorage::new(storage, PREFIX_ORGANISERS_EVENTS)
        }
    }

    // Store events
    pub fn store_events(& mut self, organiser: &CanonicalAddr, events: &Vec<u128>) {
        self.storage.set(&organiser.to_string().as_bytes(), &bincode::serialize(events).unwrap());
    }    

    // Load an organisers events
    pub fn load_events(&self, organiser: &CanonicalAddr) -> Vec<u128> {
        match self.storage.get(&organiser.to_string().as_bytes()) {
            Some(events_bytes) => bincode::deserialize(&events_bytes).unwrap(),
            None => vec![]
        }
    }
}

// Struct to handle READONLY interaction with organisers events
pub struct ReadonlyOrganisersEvents<'a> {
    storage: ReadonlyPrefixedStorage<'a>
}

impl<'a> ReadonlyOrganisersEvents<'a> {

    // Retrieve prefixed storage
    pub fn from_storage(storage: &'a dyn Storage) -> Self {
        Self {
            storage: ReadonlyPrefixedStorage::new(storage, PREFIX_ORGANISERS_EVENTS)
        }
    }

    // Load an organisers events
    pub fn load_events(&self, organiser: &CanonicalAddr) -> Vec<u128> {
        match self.storage.get(&organiser.to_string().as_bytes()) {
            Some(events_bytes) => bincode::deserialize(&events_bytes).unwrap(),
            None => vec![]
        }
    }
}

// Struct to handle interaction with guests tickets
pub struct GuestsTickets<'a> {
    storage: PrefixedStorage<'a>
}

impl<'a> GuestsTickets<'a> {

    // Retrieve prefixed storage
    pub fn from_storage(storage: &'a mut dyn Storage) -> Self {
        Self {
            storage: PrefixedStorage::new(storage, PREFIX_GUESTS_TICKETS)
        }
    }

    // Store tickets
    pub fn store_tickets(& mut self, guest: &CanonicalAddr, tickets: &Vec<u128>) {
        self.storage.set(&guest.to_string().as_bytes(), &bincode::serialize(tickets).unwrap());
    }    

    // Load an guests tickets
    pub fn load_tickets(&self, guest: &CanonicalAddr) -> Vec<u128> {
        match self.storage.get(&guest.to_string().as_bytes()) {
            Some(tickets_bytes) => bincode::deserialize(&tickets_bytes).unwrap(),
            None => vec![]
        }
    }
}

// Struct to handle READONLY interaction with organisers events
pub struct ReadonlyGuestsTickets<'a> {
    storage: ReadonlyPrefixedStorage<'a>
}

impl<'a> ReadonlyGuestsTickets<'a> {

    // Retrieve prefixed storage
    pub fn from_storage(storage: &'a dyn Storage) -> Self {
        Self {
            storage: ReadonlyPrefixedStorage::new(storage, PREFIX_GUESTS_TICKETS)
        }
    }

    // Load an guests tickets
    pub fn load_tickets(&self, guest: &CanonicalAddr) -> Vec<u128> {
        match self.storage.get(&guest.to_string().as_bytes()) {
            Some(tickets_bytes) => bincode::deserialize(&tickets_bytes).unwrap(),
            None => vec![]
        }
    }
}

// Helper function to convert slice of u8 to u128
fn slice_to_u128(data: &[u8]) -> StdResult<u128> {
    match <[u8; 16]>::try_from(data) {
        Ok(bytes) => Ok(u128::from_be_bytes(bytes)),
        Err(_) => Err(StdError::generic_err(
            "Corrupted data found. 16 byte expected.",
        )),
    }
}