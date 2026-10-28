mod cmd;
mod db;
mod resp;

use std::io;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use db::Db;
use resp::{Frame, ParseError};

#[tokio::main]
async fn main() -> io::Result<()> {
    // 6380 so it can run next to a real Redis on 6379.
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:6380".into());
    let listener = TcpListener::bind(&addr).await?;
    println!("werkv listening on {addr}");

    let db = Db::default();
    loop {
        let (socket, peer) = listener.accept().await?;
        let db = db.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(socket, db).await {
                eprintln!("{peer}: {e}");
            }
        });
    }
}

async fn handle(mut socket: TcpStream, db: Db) -> io::Result<()> {
    let mut buf = Vec::with_capacity(4096);
    let mut out = Vec::new();

    loop {
        // Clients can pipeline, so answer every complete frame before reading again.
        loop {
            match resp::parse(&buf) {
                Ok((frame, used)) => {
                    buf.drain(..used);
                    cmd::execute(frame, &db).write_to(&mut out);
                }
                Err(ParseError::Incomplete) => break,
                Err(ParseError::Invalid(msg)) => {
                    Frame::Error(format!("ERR Protocol error: {msg}")).write_to(&mut out);
                    return socket.write_all(&out).await;
                }
            }
        }
        if !out.is_empty() {
            socket.write_all(&out).await?;
            out.clear();
        }
        if socket.read_buf(&mut buf).await? == 0 {
            return Ok(());
        }
    }
}
