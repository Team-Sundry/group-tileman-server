mod command;
mod peer;
mod state;
mod tile;

use parking_lot::Mutex;
use std::{
    error::Error,
    fs::File,
    io::{Cursor, Read, Write},
    net::SocketAddr,
    path::Path,
    sync::Arc,
};
use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, TcpStream},
    signal,
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

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                let state = state.lock();
                let serialized = serde_json::to_vec(&*state).unwrap();
                let mut file = File::create("save.dat")?;
                file.write_all(&serialized)?;
                break
            }
            socket = listener.accept() => handle_connection(socket?, Arc::clone(&state)),
        }
    }

    Ok(())
}

fn handle_connection(socket: (TcpStream, SocketAddr), state: Arc<Mutex<State>>) {
    let (mut socket, addr) = socket;

    tokio::spawn(async move {
        let mut peer = Peer::new(Arc::clone(&state), addr).unwrap();
        let mut buf = vec![0u8; 1024 * 32];
        let mut buf_send = vec![0u8; 1024 * 32];

        'thread: loop {
            tokio::select! {
                command = peer.rx.recv() => {
                    command.expect("wtf?").send(&mut buf_send, &mut socket).await.expect("hmm");
                },
                result = socket.read(&mut buf) => {
                    let n = if result.is_err() {
                        state.lock().peers.remove(&addr);
                        println!("Player {} abruptly disconnected", if let Some(id) = peer.id { id as i16 } else { -1 });
                        break 'thread;
                    } else { result.unwrap() };
                    let cursor = &mut Cursor::new(&buf[0..n]);

                    while (cursor.position() as usize) < n {
                        let command: Command = {
                            let res = cursor.try_into();
                            if res.is_ok() { res.unwrap() } else { continue 'thread; }
                        };

                        println!("{:?} from {:?}", command, addr);

                        let result = match command {
                            Command::Disconnect => {
                                state.lock().peers.remove(&addr);
                                break 'thread;
                            },
                            _ => handle_command(command, &mut peer, &mut buf_send, &mut socket, Arc::clone(&state)).await
                        };

                        if result.is_ok() {
                            if let Some(test) = result.unwrap() {
                                println!("Warning: {}", test);
                            }
                        } else {
                            println!("{:?}", result.err());
                        }
                    }
                }
            }
        }
    });
}

async fn handle_command(
    command: Command,
    peer: &mut Peer,
    buf: &mut Vec<u8>,
    socket: &mut TcpStream,
    state: Arc<Mutex<State>>,
) -> Result<Option<String>, Box<dyn Error>> {
    match command {
        Command::Handshake(ver, id) => {
            if ver != crate::command::VERSION {
                Command::Response(Status::VersionMismatch)
                    .send(buf, socket)
                    .await?;
                return Ok(Some("Protocol version missmatch".into()));
            }
            if peer.is_registered() {
                Command::Response(Status::Err).send(buf, socket).await?;
                return Ok(Some("Handshake sent from registered client".into()));
            }
            peer.register(id, Arc::clone(&state));
            Command::Handshaken(Status::OK, peer.id.unwrap())
                .send(buf, socket)
                .await?;

            let map = state.lock().map.clone();
            for (region, tiles) in map {
                for tile in tiles {
                    Command::UpdateTile(tile.player, region, tile.x, tile.y, tile.z)
                        .send(buf, socket)
                        .await?;
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
