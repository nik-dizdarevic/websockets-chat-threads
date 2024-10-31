use std::collections::HashMap;
use std::error::Error;
use std::io::{BufReader, BufWriter, Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use bytes::{Buf, BytesMut};
use uuid::Uuid;
use websockets::{FragmentedMessage, Frame, Request, StatusCode, VecExt};
use websockets_chat_threads::ThreadPool;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
type ResponseFrame = Vec<u8>;
type Receiver<T> = mpsc::Receiver<T>;
type Sender<T> = mpsc::Sender<T>;
type Users = HashMap<Uuid, Sender<ResponseFrame>>;

#[derive(Debug)]
enum Event {
    NewUser(Uuid, BufWriter<TcpStream>),
    Message(ResponseFrame, Recipient),
}

#[derive(Debug)]
enum Recipient {
    All,
    User(Uuid),
}

fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:7878")?;
    let pool = ThreadPool::new(1024);
    println!("Listening on 127.0.0.1:7878");

    let (broker_tx, broker_rx) = mpsc::channel();
    thread::spawn(move || {
        broker_loop(broker_rx).expect("Broker failure");
    });

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        let broker_tx = broker_tx.clone();
        pool.execute(move || {
            handle_connection(stream, broker_tx).expect("Failure when handling connection");
        });
    }

    Ok(())
}

fn handle_connection(mut stream: TcpStream, mut broker_tx: Sender<Event>) -> Result<()> {
    let user = Uuid::new_v4();
    let mut buffer = BytesMut::zeroed(4096);

    // println!("Welcome user {:?}", user);

    if 0 == stream.read(&mut buffer)? {
        return Err("Connection closed by remote.".into());
    }

    let request = Request::new(&buffer)?;
    if let Some(response) = request.response() {
        stream.write_all(response.as_bytes())?;
        stream.flush()?;
        buffer.clear();
        let result = read_loop(user, stream, buffer, &mut broker_tx);
        if result.is_err() {
            let close = Frame::Close(StatusCode::ProtocolError);
            broker_tx.send(Event::Message(close.response().unwrap(), Recipient::User(user))).unwrap();
        }
        result
    } else {
        Err("Not a valid websocket request".into())
    }
}

fn read_loop(
    user: Uuid,
    stream: TcpStream,
    mut buffer: BytesMut,
    broker_tx: &mut Sender<Event>
) -> Result<()> {
    let (mut rd, wr) = (BufReader::new(stream.try_clone()?), BufWriter::new(stream));

    broker_tx.send(Event::NewUser(user, wr)).unwrap();

    let mut read_buffer = vec![0; 4096];
    let mut fragmented_message = FragmentedMessage::Text(vec![]);
    loop {
        let mut buff = Cursor::new(&buffer[..]);
        match Frame::parse(&mut buff, &mut fragmented_message) {
            Ok(frame) => {
                if let Some(response) = frame.response() {
                    if frame.is_text() || frame.is_binary() || frame.is_continuation() {
                        broker_tx.send(Event::Message(response, Recipient::All)).unwrap();
                        fragmented_message = FragmentedMessage::Text(vec![]);
                    } else {
                        broker_tx.send(Event::Message(response, Recipient::User(user))).unwrap();
                        if frame.is_close() {
                            return Ok(());
                        }
                    }
                }
                buffer.advance(buff.position() as usize);
            }
            Err(_) => {
                let n = rd.read(&mut read_buffer)?;
                if n == 0 {
                    return Err("Connection closed by remote.".into());
                }
                buffer.extend_from_slice(&read_buffer[..n]);
            }
        }
    }
}

fn writer_loop(user_rx: Receiver<ResponseFrame>, mut wr: BufWriter<TcpStream>) -> Result<()> {
    while let Ok(message) = user_rx.recv() {
        wr.write_all(&message)?;
        wr.flush()?;
        if message.is_close() {
            break;
        }
    }
    Ok(())
}

fn broker_loop(broker_rx: Receiver<Event>) -> Result<()> {
    let (disconnect_tx, disconnect_rx) = mpsc::channel();
    let mut users = Users::new();

    loop {
        let user = disconnect_rx.try_recv();
        let event = broker_rx.try_recv();

        match user {
            Ok(user) => {
                // println!("Goodbye user: {:?}", user);
                users.remove(&user);
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => break
        }

        match event {
            Ok(event) => {
                match event {
                    Event::NewUser(user, wr) => {
                        let (user_tx, user_rx) = mpsc::channel();
                        users.insert(user, user_tx);
                        let disconnect_tx = disconnect_tx.clone();
                        thread::spawn(move || {
                            let result = writer_loop(user_rx, wr);
                            disconnect_tx.send(user).unwrap();
                            result
                        });
                    }
                    Event::Message(message, recipient) => match recipient {
                        Recipient::All => {
                            for user_tx in users.values() {
                                if let Err(e) = user_tx.send(message.clone()) {
                                    eprintln!("Failed sending to other users: {}", e);
                                }
                            }
                        }
                        Recipient::User(user) => {
                            if let Some(user_tx) = users.get(&user) {
                                user_tx.send(message).unwrap();
                            }
                        }
                    }
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => break
        }
    }

    Ok(())
}