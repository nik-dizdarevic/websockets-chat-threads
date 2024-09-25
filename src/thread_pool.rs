use std::thread;
use std::thread::JoinHandle;
use std::sync::{Arc, mpsc, Mutex};

type Sender<T> = mpsc::Sender<T>;
type Receiver<T> = mpsc::Receiver<T>;
type Job = Box<dyn FnOnce() + Send + 'static>;

pub struct ThreadPool {
    workers: Vec<Worker>,
    tx: Option<Sender<Job>>,
}

struct Worker {
    // id: usize,
    thread: Option<JoinHandle<()>>
}

impl ThreadPool {
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        let (tx, rx) = mpsc::channel();
        let rx = Arc::new(Mutex::new(rx));

        let mut workers = Vec::with_capacity(size);
        (0..size).for_each(|id| workers.push(Worker::new(id, rx.clone())));

        ThreadPool {
            workers,
            tx: Some(tx),
        }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.tx.as_ref().unwrap().send(job).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.tx.take());
        for worker in &mut self.workers {
            // println!("Shutting down worker {:?}", worker.id);
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

impl Worker {
    fn new(_id: usize, rx: Arc<Mutex<Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let job = rx.lock().unwrap().recv();
            match job {
                Ok(job) => {
                    // println!("Worker {id} got a job; executing.");
                    job();
                }
                Err(_) => {
                    // println!("Worker {id} disconnected; shutting down.");
                    break;
                }
            }
        });
        Worker {
            // id,
            thread: Some(thread),
        }
    }
}