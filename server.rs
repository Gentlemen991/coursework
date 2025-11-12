use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::{atomic::{AtomicUsize, Ordering}, Arc, Mutex};
use std::thread;

type ClientId = usize;

struct Client {
    name: String,
    stream: TcpStream,
}

fn log_and_file(logfile: &Arc<Mutex<std::fs::File>>, text: &str) {
    println!("{}", text);
    if let Ok(mut f) = logfile.lock() {
        let _ = writeln!(f, "{}", text);
    }
}

fn broadcast(clients: &Arc<Mutex<HashMap<ClientId, Client>>>, logfile: &Arc<Mutex<std::fs::File>>, msg: &str, except: Option<ClientId>) {
    let mut to_remove = Vec::new();

    let clones: Vec<(ClientId, String, TcpStream)> = {
        let guard = clients.lock().unwrap();
        guard.iter()
            .filter(|(&id, _)| Some(id) != except)
            .map(|(&id, c)| (id, c.name.clone(), c.stream.try_clone().expect("clone stream")))
            .collect()
    };

    for (id, name, mut s) in clones {
        if let Err(e) = s.write_all(msg.as_bytes()) {
            log_and_file(logfile, &format!("Ошибка отправки {}(id={}) : {}", name, id, e));
            to_remove.push(id);
        }
    }

    if !to_remove.is_empty() {
        let mut guard = clients.lock().unwrap();
        for id in to_remove {
            if let Some(c) = guard.remove(&id) {
                log_and_file(logfile, &format!("[SERVER] {} (id={}) disconnected (write error)", c.name, id));
            }
        }
    }
}

fn handle_client(id: ClientId, mut stream: TcpStream, clients: Arc<Mutex<HashMap<ClientId, Client>>>, logfile: Arc<Mutex<std::fs::File>>) {
    let peer = stream.peer_addr().map(|a| a.to_string()).unwrap_or_else(|_| "?".to_string());
    log_and_file(&logfile, &format!("New connection: id={} peer={}", id, peer));

    if let Err(e) = stream.write_all(b"Enter your name: ") {
        log_and_file(&logfile, &format!("Failed to prompt name for id={} : {}", id, e));
        let _ = stream.shutdown(Shutdown::Both);
        return;
    }

    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut name = String::new();
    match reader.read_line(&mut name) {
        Ok(0) => {
            log_and_file(&logfile, &format!("Client id={} closed before sending name", id));
            let _ = stream.shutdown(Shutdown::Both);
            return;
        }
        Ok(_) => {
            name = name.trim().to_string();
            if name.is_empty() {
                let _ = stream.write_all(b"Name cannot be empty\n");
                let _ = stream.shutdown(Shutdown::Both);
                return;
            }
        }
        Err(e) => {
            log_and_file(&logfile, &format!("Error reading name from id={} : {}", id, e));
            let _ = stream.shutdown(Shutdown::Both);
            return;
        }
    }

    {
        let mut guard = clients.lock().unwrap();
        guard.insert(id, Client { name: name.clone(), stream: stream.try_clone().expect("clone") });
    }

    let welcome = format!("[SERVER] Welcome, {}! Use /quit to exit, /list to see users\n", name);
    let _ = stream.write_all(welcome.as_bytes());
    broadcast(&clients, &logfile, &format!("[SERVER] {} has joined\n", name), Some(id));

    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                let text = line.trim().to_string();
                if text.is_empty() { continue; }

                log_and_file(&logfile, &format!("Message from {} (id={}): {}", name, id, text));

                if text.eq_ignore_ascii_case("/quit") {
                    let _ = stream.write_all(b"[SERVER] Bye\n");
                    break;
                } else if text.eq_ignore_ascii_case("/list") {
                    let guard = clients.lock().unwrap();
                    let users: Vec<String> = guard.values().map(|c| c.name.clone()).collect();
                    let reply = format!("[SERVER] Users: {}\n", users.join(", "));
                    let _ = stream.write_all(reply.as_bytes());
                } else {
                    let out = format!("{}: {}\n", name, text);
                    broadcast(&clients, &logfile, &out, None);
                }
            }
            Err(e) => {
                log_and_file(&logfile, &format!("Error reading from {} (id={}): {}", name, id, e));
                break;
            }
        }
    }

    {
        let mut guard = clients.lock().unwrap();
        guard.remove(&id);
    }

    log_and_file(&logfile, &format!("[SERVER] {} (id={}) disconnected", name, id));
    broadcast(&clients, &logfile, &format!("[SERVER] {} has left\n", name), Some(id));
    let _ = stream.shutdown(Shutdown::Both);
}

fn main() -> std::io::Result<()> {
    let host = "0.0.0.0";
    let port = 12345;
    let bind = format!("{}:{}", host, port);
    println!("Starting server on {}", bind);

    let logfile = Arc::new(Mutex::new(
        OpenOptions::new().create(true).append(true).open("server.log")?
    ));

    let listener = TcpListener::bind(&bind)?;

    let clients: Arc<Mutex<HashMap<ClientId, Client>>> = Arc::new(Mutex::new(HashMap::new()));
    let id_counter = Arc::new(AtomicUsize::new(1));

    log_and_file(&logfile, &format!("Server listening on {}", bind));

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let clients_c = Arc::clone(&clients);
                let logfile_c = Arc::clone(&logfile);
                let id = id_counter.fetch_add(1, Ordering::SeqCst);
                thread::spawn(move || {
                    handle_client(id, s, clients_c, logfile_c);
                });
            }
            Err(e) => {
                log_and_file(&logfile, &format!("Accept error: {}", e));
            }
        }
    }

    Ok(())
}