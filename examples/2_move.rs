use std::thread;

fn main() {
    let v = vec![1, 2, 3];

    // uporabimo move
    let handle = thread::spawn(move || {
        println!("Here's a vector: {v:?}");
    });

    handle.join().unwrap();
}