use std::collections::BTreeMap;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use crate::data::{Status, Ticket, TicketDraft};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TicketId(pub u64);

#[derive(Clone)]
pub struct TicketStore {
    lock: Arc<RwLock<TicketStoreInternal>>,
}

pub struct TicketStoreInternal {
    tickets: BTreeMap<TicketId, Arc<RwLock<Ticket>>>,
    counter: u64,
}

pub struct TicketStoreReader<'a> {
    store: RwLockReadGuard<'a, TicketStoreInternal>,
}

pub struct TicketStoreWriter<'a> {
    store: RwLockWriteGuard<'a, TicketStoreInternal>,
}

impl TicketStoreReader<'_> {
    pub fn get(&self, id: TicketId) -> Option<Arc<RwLock<Ticket>>> {
        self.store.tickets.get(&id).cloned()
    }
}

impl TicketStoreWriter<'_> {
    pub fn add_ticket(&mut self, ticket: TicketDraft) -> TicketId {
        let id = TicketId(self.store.counter);
        self.store.counter += 1;
        let ticket = Ticket {
            id,
            title: ticket.title,
            description: ticket.description,
            status: Status::ToDo,
        };
        let ticket = Arc::new(RwLock::new(ticket));
        self.store.tickets.insert(id, ticket);
        id
    }
}

impl TicketStore {
    pub fn new() -> Self {
        let internal = TicketStoreInternal {
            tickets: BTreeMap::new(),
            counter: 0,
        };

        Self {
            lock: Arc::new(RwLock::new(internal)),
        }
    }
    pub async fn read(&self) -> TicketStoreReader {
        TicketStoreReader { store: self.lock.read().await }
    }

    pub async fn write(&self) -> TicketStoreWriter {
        TicketStoreWriter { store: self.lock.write().await }
    }
}
