use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::env;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug)]
struct ConnectionWatcher {
    connections: HashMap<String, bool>,
}

impl ConnectionWatcher {
    fn new(remote_ports: &Vec<String>) -> Self {
        let mut conn = HashMap::new();
        for port in remote_ports {
            conn.insert(port.clone(), false);
        }
        Self { connections: conn }
    }

    fn are_all_conected(&self) -> bool {
        for (_, is_connected) in &self.connections {
            if !is_connected {
                return false;
            }
        }
        return true;
    }

    fn set_connection(&mut self, port: String, new_state: bool) {
        self.connections.insert(port, new_state);
    }

    fn is_port_connected(&self, port: &String) -> bool {
        *self.connections.get(port).unwrap()
    }
}

async fn read_from_connection(mut stream: TcpStream) {
    let mut buffer = [0; 1024];
    loop {
        let result_stream = stream.read(&mut buffer).await;
        match result_stream {
            Ok(size) if size == 0 => {
                eprintln!("Connection closed by the client");
                break;
            }
            Ok(size) => {
                let received_data = &buffer[..size];
                println!(
                    "Received data from {}: {:?}",
                    stream.peer_addr().unwrap(),
                    received_data
                );
            }
            Err(msg) => {
                eprintln!("Error in connection: {}", msg);
                break;
            }
        }
    }
}

async fn open_listener(local_port: &String) -> TcpListener {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", local_port))
        .await
        .unwrap();
    println!("Server listening on port {}", local_port);
    listener
}

async fn listen_for_remote_ports(
    local_port: String,
    sender_connections: tokio::sync::mpsc::Sender<Vec<TcpStream>>,
) {
    let listener = open_listener(&local_port).await;
    let mut connections = Vec::new();
    let (stream_connection, socket_addr) = listener.accept().await.unwrap();
    connections.push(stream_connection);

    println!("Received connection.");
    sender_connections.send(connections).await.unwrap();
}

async fn connect_with_remote(port: String, conn_watcher: Arc<Mutex<ConnectionWatcher>>) {
    println!("Trying to stablish connection with {}", port);
    let mut connection_successful = false;
    while !connection_successful {
        let stream_remote_result = TcpStream::connect(format!("127.0.0.1:{}", port)).await;
        connection_successful = match stream_remote_result {
            Ok(_) => true,
            Err(_) => false,
        }
    }
    if connection_successful {
        println!("Connection successfull with {}", port);
        conn_watcher
            .lock()
            .unwrap()
            .borrow_mut()
            .set_connection(port, true);
    }
}

#[tokio::main]
async fn main() {
    // Read from arguments
    let args: Vec<String> = env::args().collect();
    let local_port = args[1].clone(); // Clone the port string
    let remote_ports = vec![args[2].clone(), args[3].clone()];

    // Read the input to send to the other parties
    let input = args[4].clone();

    let (sender_conn, mut receiver_conn) = tokio::sync::mpsc::channel(1);

    let listening_port_thread =
        tokio::spawn(listen_for_remote_ports(local_port.clone(), sender_conn));

    let mut connection_threads_remote = Vec::new();

    let conn_watcher = Arc::new(Mutex::new(ConnectionWatcher::new(&remote_ports)));
    for port in remote_ports {
        let shared_conn_watcher = Arc::clone(&conn_watcher);
        let thread_remote = tokio::spawn(connect_with_remote(port, shared_conn_watcher));
        connection_threads_remote.push(thread_remote);
    }

    let input_connections_remote = receiver_conn.recv().await.unwrap();

    listening_port_thread.await.unwrap();

    for thread in connection_threads_remote {
        thread.await.unwrap();
    }
    println!("All tree parties are connected");

    println!("Connections {:?}", input_connections_remote);

    loop {}
}
