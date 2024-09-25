use std::sync::Mutex;

fn main() {
    // kreiramo ključavnico
    let m = Mutex::new(5);

    {
        // pridobimo zaklep
        // blokira nit
        let mut num = m.lock().unwrap();
        *num = 6;
    }

    println!("m = {m:?}");
}