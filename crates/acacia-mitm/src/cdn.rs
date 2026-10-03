//! A stand-in pack CDN for capturing how the game downloads a pack from ResourcePacksInfo's
//! `cdn_url`: every request is logged with its headers and answered with the one pack zip.

use std::io::{self, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// Serves `zip` on `port`, logging to `log`; returns the URL to hand the game.
pub fn start(zip: &Path, port: u16, log: &Path) -> io::Result<String> {
    let pack = std::fs::read(zip).map_err(|e| io::Error::new(e.kind(), format!("--pack-cdn {}: {e}", zip.display())))?;
    serve(SocketAddr::from(([0, 0, 0, 0], port)), pack, log)?;
    let url = format!("http://127.0.0.1:{port}/pack.zip");
    println!("packs point at {url}");
    Ok(url)
}

fn serve(listen: SocketAddr, pack: Vec<u8>, log: &Path) -> io::Result<()> {
    let listener = TcpListener::bind(listen)?;
    let log = Arc::new(Mutex::new(std::fs::OpenOptions::new().create(true).append(true).open(log)?));
    let pack: Arc<[u8]> = pack.into();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (pack, log) = (pack.clone(), log.clone());
            std::thread::spawn(move || {
                if let Err(e) = answer(stream, &pack, &log) {
                    println!("pack cdn: {e}");
                }
            });
        }
    });
    Ok(())
}

fn answer(stream: TcpStream, pack: &[u8], log: &Mutex<std::fs::File>) -> io::Result<()> {
    let peer = stream.peer_addr()?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut head = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line.trim_end().is_empty() {
            break;
        }
        head.push(line.trim_end().to_owned());
    }
    let millis = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_millis();
    let entry = format!("{millis} {peer}\n{}\n\n", head.join("\n"));
    print!("pack cdn request:\n{entry}");
    log.lock().unwrap_or_else(|p| p.into_inner()).write_all(entry.as_bytes())?;

    let method = head.first().and_then(|l| l.split(' ').next()).unwrap_or_default();
    let mut stream = stream;
    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/zip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", pack.len())?;
    if method != "HEAD" {
        stream.write_all(pack)?;
    }
    stream.flush()
}
