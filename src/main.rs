use std::{
    net::SocketAddr,
};

use tokio::net::TcpListener;

use tokio_modbus::{
    prelude::*,
    server::tcp::{Server, accept_tcp_connection},
};

mod modbus;
use modbus::{ModbusRegisterAccess, ModbusService};

mod model;
use model::{ModleHandler};

impl ModbusRegisterAccess for ModleHandler {
    fn read_input_regs(&self, _addr: u16, _cnt: u16) -> Result<Vec<u16>, ExceptionCode> {
        let regs = self.input_registers.lock().unwrap();
        if _addr + _cnt > regs.len() as u16 {
            return Err(ExceptionCode::IllegalDataAddress);
        }
        Ok(regs[_addr as usize..(_addr + _cnt) as usize].to_vec())
    }

    fn read_holding_regs(&self, _addr: u16, _cnt: u16) -> Result<Vec<u16>, ExceptionCode> {
        let regs = self.input_registers.lock().unwrap();
        if _addr + _cnt > regs.len() as u16 {
            return Err(ExceptionCode::IllegalDataAddress);
        }
        Ok(regs[_addr as usize..(_addr + _cnt) as usize].to_vec())
    }

    fn write_regs(&mut self, _addr: u16, _values: &[u16]) -> Result<(), ExceptionCode> {
        println!("{:?}", _values);
        let mut hold_regs = self.holding_registers.lock().unwrap();
        if _addr as usize + _values.len() > hold_regs.len() {
            return Err(ExceptionCode::IllegalDataAddress);
        }
        hold_regs[_addr as usize..(_addr as usize + _values.len())].copy_from_slice(_values);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let handler = ModleHandler::new();
    let handler_for_update = handler.clone();
    let socket_addr: std::net::SocketAddr = "0.0.0.0:1502".parse().unwrap();
    tokio::spawn(async move {
        handler_for_update.update_loop().await;
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
