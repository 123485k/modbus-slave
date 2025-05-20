use std::{
    io::{self, Write}, net::SocketAddr
};

use tokio::{net::TcpListener, time};

use tokio_modbus::{
    server::tcp::{Server, accept_tcp_connection},
};

mod modbus;
use modbus::{ModbusService};

mod model;
use model::{ModleHandler};

mod modbus_access;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let handler = ModleHandler::new();
    let handler_for_update = handler.clone();
    let socket_addr: std::net::SocketAddr = "0.0.0.0:1502".parse().unwrap();
    tokio::spawn(async move {
        handler_for_update.update_loop().await;
    });
    let handler_for_print = handler.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(time::Duration::from_millis(100));
        loop {
            interval.tick().await;
            print!("\r{:?}", handler_for_print.get_stat());
            io::stdout().flush().unwrap();
        }
    });
    println!("Starting up server on {socket_addr}");
    let listener = TcpListener::bind(socket_addr).await?;
    let server = Server::new(listener);
    let new_service = |_socket_addr: SocketAddr| Ok(Some(ModbusService::new(handler.clone())));
    let on_connected = |stream, socket_addr| async move {
        accept_tcp_connection(stream, socket_addr, new_service)
    };
    let on_process_error = |err| {
        eprintln!("{err}");
    };
    let _ = server.serve(&on_connected, on_process_error).await;
    Ok(())
}
