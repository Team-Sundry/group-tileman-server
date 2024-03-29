use crate::{command::Command, state::State};
use anyhow::Result;
use parking_lot::Mutex;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

pub struct Peer {
    pub uid: Option<u64>,
    pub id: Option<u8>,
    pub tx: UnboundedSender<Command>,
}

impl Peer {
    pub fn new(
        state: Arc<Mutex<State>>,
        addr: SocketAddr,
    ) -> Result<(Self, UnboundedReceiver<Command>)> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut state = state.lock();
        state.peers.insert(addr, tx.clone());
        Ok((
            Peer {
                uid: None,
                id: None,
                tx,
            },
            rx,
        ))
    }

    pub fn is_registered(&self) -> bool {
        self.uid.is_some()
    }

    pub fn register(&mut self, uid: u64, state: Arc<Mutex<State>>) {
        self.uid = Some(uid);
        let mut state = state.lock();
        if let Some(id) = state.id_map.get(&uid) {
            self.id = Some(*id);
        } else {
            let id = state.id_map.len();
            state.id_map.insert(uid, id as u8);
            self.id = Some(id as u8);
        }
    }

    pub fn send(&self, command: Command) {
        self.tx.send(command).unwrap();
    }
}
