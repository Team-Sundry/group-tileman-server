mod command;
mod peer;
mod state;
mod tile;

use parking_lot::{Mutex, MutexGuard};
use std::{
    error::Error,
    fs::File,
    io::{Cursor, Read, Write},
    net::SocketAddr,
    path::Path,
    sync::Arc, time::Duration,
};
use tokio::{
    io::{split, AsyncReadExt},
    net::{TcpListener, TcpStream},
    signal, time,
};

use crate::{
    command::{Command, Status},
    peer::Peer,
    state::State,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = "0.0.0.0:2532".to_string();
    let listener: TcpListener = TcpListener::bind(&addr).await?;

    let state = if Path::new("./save.dat").exists() {
        println!("Loading save file...");
        let mut contents = String::new();
        File::open("save.dat")?.read_to_string(&mut contents)?;
        let state = Arc::new(Mutex::new(serde_json::from_str(contents.as_str())?));
        println!("Save file loaded!");
        state
    } else {
        Arc::new(Mutex::new(State::new()))
    };

    println!("Listening on: {}", addr);
    
    let state_clone = Arc::clone(&state);
    let save_handle = tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(60 * 60));
        interval.tick().await;
        loop {
            interval.tick().await;
            println!("Saving...");
            let state = state_clone.lock();
            save(state);
            println!("Map saved!");
        }
    });

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                save_handle.abort();
                let state = state.lock();
                state.broadcast_all(Command::Disconnect);
                save(state);
                break
            }
            socket = listener.accept() => handle_connection(socket?, Arc::clone(&state)),
        }
    }

    Ok(())
}

fn handle_connection(socket: (TcpStream, SocketAddr), state: Arc<Mutex<State>>) {
    let (stream, addr) = socket;
    let (mut reciever, mut sender) = split(stream);
    let (mut peer, mut rx) = Peer::new(Arc::clone(&state), addr).unwrap();

    tokio::spawn(async move {
        let mut buf = vec![0u8; 1024 * 32];

        while let Some(command) = rx.recv().await {
            _ = command.send(&mut buf, &mut sender).await;
        }
    });

    tokio::spawn(async move {
        let mut buf = vec![0u8; 1024 * 32];

        'thread: loop {
            let (mut cursor, n) = if let Ok(bytes) = reciever.read(&mut buf).await {
                (Cursor::new(&buf[0..bytes]), bytes)
            } else {
                state.lock().peers.remove(&addr);
                println!(
                    "Player {} abruptly disconnected",
                    if let Some(id) = peer.id {
                        id as i16
                    } else {
                        -1
                    }
                );
                break 'thread;
            };

            while (cursor.position() as usize) < n {
                let command: Command = if let Ok(command) = (&mut cursor).try_into() {
                    command
                } else {
                    continue 'thread;
                };

                println!("{:?} from {:?}", command, addr);

                let result = match command {
                    Command::Disconnect => {
                        state.lock().peers.remove(&addr);
                        break 'thread;
                    }
                    _ => handle_command(command, &mut peer, Arc::clone(&state)).await,
                };

                if let Ok(result) = result {
                    if let Some(msg) = result {
                        println!("Warning: {}", msg);
                    }
                } else {
                    println!("{:?}", result.err());
                }
            }
        }
    });
}

async fn handle_command(
    command: Command,
    peer: &mut Peer,
    state: Arc<Mutex<State>>,
) -> Result<Option<String>, Box<dyn Error>> {
    match command {
        Command::Handshake(ver, id) => {
            if ver != crate::command::VERSION {
                peer.send(Command::Response(Status::VersionMismatch));
                return Ok(Some("Protocol version missmatch".into()));
            }
            if peer.is_registered() {
                peer.send(Command::Response(Status::Err));
                return Ok(Some("Handshake sent from registered client".into()));
            }
            peer.register(id, Arc::clone(&state));
            peer.send(Command::Handshaken(Status::OK, peer.id.unwrap()));

            let map = state.lock().map.clone();
            for (region, tiles) in map {
                for tile in tiles {
                    peer.send(Command::UpdateTile(
                        tile.player,
                        region,
                        tile.x,
                        tile.y,
                        tile.z,
                    ));
                }
            }
        }
        Command::PlaceTile(region, x, y, z) => {
            let mut state = state.lock();
            let result = state.insert_tile(peer.id.unwrap(), region, x, y, z);
            if result.is_ok() {
                state.broadcast_all(Command::UpdateTile(peer.id.unwrap(), region, x, y, z));
            }
        }
        _ => {}
    }

    Ok(None)
}

fn save(state: MutexGuard<State>) {
    let serialized = serde_json::to_vec(&*state).unwrap();
    let mut file = File::create("save.dat").unwrap();
    file.write_all(&serialized).unwrap();
}
