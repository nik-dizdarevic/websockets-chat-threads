use std::thread;

fn main() {
    // kreiramo nit
    thread::spawn(|| {
        println!("Hello from a thread!");
    });
}