use std::env;
use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::thread;

fn receive_loop(stream: TcpStream) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => {
                println!("[INFO] Server closed connection");
                break;
            }
            Ok(_) => print!("{}", line),
            Err(e) => {
                eprintln!("[ERROR] receive_loop: {}", e);
                break;
            }
        }
    }
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let host = if args.len() > 1 { &args[1] } else { "127.0.0.1" };
    let port = if args.len() > 2 { &args[2] } else { "12345" };
    let addr = format!("{}:{}", host, port);

    println!("Connecting to {}", addr);
    let mut stream = TcpStream::connect(&addr)?;
    let stream_for_recv = stream.try_clone()?;

    thread::spawn(move || receive_loop(stream_for_recv));

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut prompt = String::new();
    reader.read_line(&mut prompt).ok();
    print!("{}", prompt);
    io::stdout().flush().ok();

    let mut stdin = io::stdin().lock();
    let mut name = String::new();
    stdin.read_line(&mut name)?;
    let name = name.trim().to_string();
    let name = if name.is_empty() { "guest".to_string() } else { name };
    stream.write_all(format!("{}\n", name).as_bytes())?;

    let mut input = String::new();
    loop {
        input.clear();
        let bytes = stdin.read_line(&mut input)?;
        if bytes == 0 { break; }
        let text = input.trim().to_string();
        if text.is_empty() { continue; }
        stream.write_all(format!("{}\n", text).as_bytes())?;
        if text.eq_ignore_ascii_case("/quit") { break; }
    }

    println!("[INFO] Client exiting");
    Ok(())
}