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
    fn write_regs(&self, addr: u16, values: &[u16]) -> Result<(), ExceptionCode>;
}
struct ModbusService<T: ModbusRegisterAccess> {
    modbus_handler: T,
}

impl<T: ModbusRegisterAccess> tokio_modbus::server::Service for ModbusService<T> {
    type Request = Request<'static>;
    type Response = Response;
    type Exception = ExceptionCode;
    type Future = future::Ready<Result<Self::Response, Self::Exception>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let res = match req {
            Request::ReadInputRegisters(addr, cnt) => self
                .modbus_handler
                .read_input_regs(addr, cnt)
                .map(Response::ReadInputRegisters),
            Request::ReadHoldingRegisters(addr, cnt) => self
                .modbus_handler
                .read_holding_regs(addr, cnt)
                .map(Response::ReadHoldingRegisters),
            Request::WriteSingleRegister(addr, value) => self
                .modbus_handler
                .write_regs(addr, std::slice::from_ref(&value))
                .map(|_| Response::WriteSingleRegister(addr, value)),
            Request::WriteMultipleRegisters(addr, values) => self
                .modbus_handler
                .write_regs(addr, &values)
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

impl<T: ModbusRegisterAccess> ModbusService<T> {
    fn new(handle:T) -> Self{
        Self { modbus_handler: handle }
    }
}

#[tokio::main]
async fn main() {
    print!("hello\n");
}
