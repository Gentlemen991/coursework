use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

fn handle_client(mut stream: TcpStream) {
    let mut buffer = [0; 512];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                println!("Клиент отключился");
                break;
            }
            Ok(n) => {
                let input = String::from_utf8_lossy(&buffer[..n]);
                println!("От клиента: {}", input);
                let response = format!("Вы сказали: {}", input);
                stream.write_all(response.as_bytes()).unwrap();
            }
            Err(e) => {
                eprintln!("Ошибка: {}", e);
                break;
            }
        }
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").expect("Не удалось запустить сервер");
    println!("Сервер запущен на 127.0.0.1:7878");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(|| handle_client(stream));
            }
            Err(e) => {
                eprintln!("Ошибка соединения: {}", e);
            }
        }
    }
}