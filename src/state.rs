use crate::{command::Command, tile::Tile};
use anyhow::anyhow;
use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::{collections::HashMap, net::SocketAddr};
use tokio::sync::mpsc::UnboundedSender;

pub type Map = HashMap<i32, Vec<Tile>>;

#[derive(Serialize, Deserialize)]
pub struct State {
    #[serde(skip)]
    pub peers: HashMap<SocketAddr, UnboundedSender<Command>>,
    pub id_map: HashMap<u64, u8>,
    pub map: Map,
}

impl State {
    pub fn new() -> Self {
        State {
            peers: HashMap::new(),
            id_map: HashMap::new(),
            map: HashMap::new(),
        }
    }

    pub fn insert_tile(
        &mut self,
        player: u8,
        region_id: i32,
        x: i32,
        y: i32,
        z: i32,
    ) -> Result<()> {
        if let Some(region) = self.map.get_mut(&region_id) {
            if region
                .iter()
                .any(|tile: &Tile| tile.x == x && tile.y == y && tile.z == z)
            {
                return Err(anyhow!("Tile in database"));
            }

            region.push(Tile {
                x,
                y,
                z,
                player,
                timestamp: Utc::now().timestamp(),
            })
        } else {
            self.map.insert(
                region_id,
                vec![Tile {
                    x,
                    y,
                    z,
                    player,
                    timestamp: Utc::now().timestamp(),
                }],
            );
        }

        Ok(())
    }

    pub fn broadcast_all(&mut self, message: Command) {
        for peer in self.peers.iter_mut() {
            let _ = peer.1.send(message.clone().into());
        }
    }
}
