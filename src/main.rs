use std::{
    borrow::Cow,
    collections::{HashMap, btree_map::Values},
    future,
    net::{AddrParseError, SocketAddr},
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::{net::TcpListener, runtime::Handle};

trait PhyModel {}

use tokio_modbus::{
    prelude::*,
    server::tcp::{Server, accept_tcp_connection},
};

struct ModelConfig {
    r_tank: f64,
    h_tank: f64,
    r_out: f64,
    h_liquid: f64,
    v_liquid: f64,
}

struct ModelParams {
    val_a: f64,
    val_b: f64,
    val_out: f64,
}

struct ModelStat {
    config: ModelConfig,
    paras: ModelParams,
}

impl ModelStat {
    pub fn new(config: ModelConfig, paras: ModelParams) -> Self {
        Self { config, paras }
    }
}
trait ModbusRegisterAccess {
    fn read_input_regs(&self, addr: u16, cnt: u16) -> Result<Vec<u16>, ExceptionCode>;
    fn read_holding_regs(&self, addr: u16, cnt: u16) -> Result<Vec<u16>, ExceptionCode>;
    fn write_regs(&mut self, addr: u16, values: &[u16]) -> Result<(), ExceptionCode>;
}
struct ModbusService<T: ModbusRegisterAccess + Send + 'static> {
    modbus_handler: Arc<Mutex<T>>,
}

impl<T: ModbusRegisterAccess + Send + 'static> tokio_modbus::server::Service for ModbusService<T> {
    type Request = Request<'static>;
    type Response = Response;
    type Exception = ExceptionCode;
    type Future = future::Ready<Result<Self::Response, Self::Exception>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let mut handler = self.modbus_handler.lock().unwrap();
        let res = match req {
            Request::ReadInputRegisters(addr, cnt) =>
                handler.read_input_regs(addr, cnt)
                    .map(Response::ReadInputRegisters),
            Request::ReadHoldingRegisters(addr, cnt) =>
                handler.read_holding_regs(addr, cnt)
                    .map(Response::ReadHoldingRegisters),
            Request::WriteSingleRegister(addr, value) =>
                handler.write_regs(addr, std::slice::from_ref(&value))
                    .map(|_| Response::WriteSingleRegister(addr, value)),
            Request::WriteMultipleRegisters(addr, values) =>
                handler.write_regs(addr, &values)
                    .map(|_| Response::WriteMultipleRegisters(addr, values.len() as u16)),
            _ => {
                println!(
                    "SERVER: Exception::IllegalFunction - Unimplemented function code in request: {req:?}"
                );
                Err(ExceptionCode::IllegalFunction)
            }
        };
        future::ready(res)
    }
}

impl<T: ModbusRegisterAccess + Send + 'static> ModbusService<T> {
    fn new(handle: T) -> Self {
        Self {
            modbus_handler: Arc::new(Mutex::new(handle)),
        }
    }
}

#[derive(Clone)]
struct TimeHandler {
    registers: Arc<Mutex<Vec<u16>>>,
    last_update: Arc<Mutex<std::time::Instant>>,
}

impl TimeHandler {
    fn new() -> Self {
        let mut registers = vec![0; 10];
        Self {
            registers: Arc::new(Mutex::new(registers)),
            last_update: Arc::new(Mutex::new(std::time::Instant::now())),
        }
    }

    async fn update_loop(&self) {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
        loop {
            interval.tick().await;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap();
            let mut regs = self.registers.lock().unwrap();
            regs[0] = (now.as_secs() >> 16) as u16;
            regs[1] = now.as_secs() as u16;
            regs[2] = now.subsec_millis() as u16;
            *self.last_update.lock().unwrap() = std::time::Instant::now();
        }
    }
}

impl ModbusRegisterAccess for TimeHandler {
    fn read_input_regs(&self, addr: u16, cnt: u16) -> Result<Vec<u16>, ExceptionCode> {
        let regs = self.registers.lock().unwrap();
        if addr + cnt > regs.len() as u16 {
            return Err(ExceptionCode::IllegalDataAddress);
        }
        Ok(regs[addr as usize..(addr + cnt) as usize].to_vec())
    }

    fn read_holding_regs(&self, _addr: u16, _cnt: u16) -> Result<Vec<u16>, ExceptionCode> {
        Err(ExceptionCode::IllegalDataAddress)
    }

    fn write_regs(&mut self, _addr: u16, _values: &[u16]) -> Result<(), ExceptionCode> {
        Err(ExceptionCode::IllegalDataAddress)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let handler = TimeHandler::new();
    let handler_for_update = handler.clone();
    let socket_addr: std::net::SocketAddr = "0.0.0.0:1502".parse().unwrap();
    tokio::spawn(async move {
        handler_for_update.update_loop().await;
    });
    println!("Starting up server on {socket_addr}");
    let listener = TcpListener::bind(socket_addr).await?;
    let server = Server::new(listener);
    let new_service = |_socket_addr:SocketAddr| Ok(Some(ModbusService::new(handler.clone())));
    let on_connected = |stream, socket_addr| async move {
        accept_tcp_connection(stream, socket_addr, new_service)
    };
    let on_process_error = |err| {
        eprintln!("{err}");
    };
    server.serve(&on_connected, on_process_error).await;
    Ok(())
}
