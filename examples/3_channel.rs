use std::sync::mpsc;
use std::thread;
use std::time::Duration;

fn main() {
    // kreiramo kanal
    let (tx, rx) = mpsc::channel();

    // kloniramo oddajnik
    let tx1 = tx.clone();
    thread::spawn(move || {
        let vals = vec![
            String::from("hi"),
            String::from("from thread 1"),
        ];

        for val in vals {
            // pošljemo vrednost
            tx1.send(val).unwrap();
            thread::sleep(Duration::from_secs(1));
        }
    });

    thread::spawn(move || {
        let vals = vec![
            String::from("hello"),
            String::from("from thread 2"),
        ];

        for val in vals {
            // pošljemo vrednost
            tx.send(val).unwrap();
            thread::sleep(Duration::from_secs(1));
        }
    });

    // sprejmemo vrednost
    // blokira glavno nit
    while let Ok(received) = rx.recv() {
        println!("Got: {}", received);
    }
}