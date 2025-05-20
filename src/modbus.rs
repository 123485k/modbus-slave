use std::future;
use tokio_modbus::{
    prelude::*,
};
use std::sync::{Arc, Mutex};

pub trait ModbusRegisterAccess {
    fn read_input_regs(&self, addr: u16, cnt: u16) -> Result<Vec<u16>, ExceptionCode>;
    fn read_holding_regs(&self, addr: u16, cnt: u16) -> Result<Vec<u16>, ExceptionCode>;
    fn write_regs(&mut self, addr: u16, values: &[u16]) -> Result<(), ExceptionCode>;
}

pub struct ModbusService<T: ModbusRegisterAccess + Send + 'static> {
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
            Request::ReadInputRegisters(addr, cnt) => handler
                .read_input_regs(addr, cnt)
                .map(Response::ReadInputRegisters),
            Request::ReadHoldingRegisters(addr, cnt) => handler
                .read_holding_regs(addr, cnt)
                .map(Response::ReadHoldingRegisters),
            Request::WriteSingleRegister(addr, value) => handler
                .write_regs(addr, std::slice::from_ref(&value))
                .map(|_| Response::WriteSingleRegister(addr, value)),
            Request::WriteMultipleRegisters(addr, values) => handler
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

impl<T: ModbusRegisterAccess + Send + 'static> ModbusService<T> {
    pub fn new(handle: T) -> Self {
        Self {
            modbus_handler: Arc::new(Mutex::new(handle)),
        }
    }
}
