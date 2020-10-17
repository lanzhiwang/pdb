use anyhow::Result;
use serde_lexpr::from_str;
use std::sync::mpsc::channel;
use std::thread;
use structopt::StructOpt;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;

#[derive(Debug, StructOpt)]
struct Config {
    #[structopt(long = "port", short = "p", env = "PDB_PORT")]
    port: u16,
}

#[derive(Debug, StructOpt)]
#[structopt(name = "pdbcli", about = "The pdb server.")]
struct Args {
    #[structopt(flatten)]
    config: Config,
}

#[paw::main]
#[tokio::main]
async fn main(args: Args) -> Result<()> {
    println!("started");

    // Create a simple streaming channel
    let (tx, rx) = channel();

    // Spawn Db handler thread
    let _ = thread::spawn(|| pdb::db::start(rx));

    let mut listener = TcpListener::bind(("127.0.0.1", args.config.port)).await?;

    loop {
        let (socket, _) = listener.accept().await?;

        let mut socket = BufReader::new(socket);

        let tx_clone = tx.clone();

        tokio::spawn(async move {
            loop {
                let mut buffer = String::new();

                match socket.read_line(&mut buffer).await {
                    Ok(_) => match from_str(&buffer) {
                        Ok(stm) => {
                            println!("Got {:?}", stm);
                            let (tx2, rx2) = channel();

                            tx_clone.send((stm, tx2)).expect("unimplemented");

                            socket
                                .write_all({
                                    let res = rx2.recv();
                                    res.expect("unimplemented").unwrap().to_string().as_bytes()
                                })
                                .await
                                .unwrap();

                            socket.flush().await.unwrap();
                        }
                        Err(e) => {
                            socket
                                .write_all(format!("No parse: {}", e).as_bytes())
                                .await
                                .unwrap();

                            socket.flush();
                        }
                    },
                    Err(e) => {
                        eprintln!("failed to read from socket; err = {:?}", e);
                        break;
                    }
                }
            }
        });
    }
}
