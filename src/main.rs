use std::{
    f64::consts::PI,
    future,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use tokio::net::TcpListener;

use tokio_modbus::{
    prelude::*,
    server::tcp::{Server, accept_tcp_connection},
};

struct ModelConfig {
    a_tank: f64,
    h_tank: f64,
    r_pipe: f64,
    h_liquid: f64,
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
    fn new(handle: T) -> Self {
        Self {
            modbus_handler: Arc::new(Mutex::new(handle)),
        }
    }
}

#[derive(Clone)]
struct ModleHandler {
    input_registers: Arc<Mutex<Vec<u16>>>,
    holding_registers: Arc<Mutex<Vec<u16>>>,
    module_stats: Arc<Mutex<ModelStat>>,
}

impl ModleHandler {
    fn get_float_from_2_u16(v: &[u16], addr: u16) -> f32 {
        let high = v[addr as usize] as u32;
        let low = v[addr as usize + 1] as u32;
        let combined = if cfg!(target_endian = "little") {
            ((low as u32) << 16) | (high as u32)
        } else {
            ((high as u32) << 16) | (low as u32)
        };
        f32::from_bits(combined)
    }

    fn float_to_2_u16_be(value: f32) -> (u16, u16) {
        let bits = value.to_bits();
        ((bits >> 16) as u16, bits as u16)
    }

    fn new() -> Self {
        let input_regs = vec![0; 10];
        let hold_regs = vec![0; 10];
        let default_config = ModelConfig {
            a_tank: 0.4,
            h_tank: 5.0,
            r_pipe: 0.25,
            h_liquid: 0.0,
        };
        let default_params = ModelParams {
            val_a: 0.0,
            val_b: 0.0,
            val_out: 0.0,
        };
        Self {
            input_registers: Arc::new(Mutex::new(input_regs.clone())),
            holding_registers: Arc::new(Mutex::new(hold_regs)),
            module_stats: Arc::new(Mutex::new(ModelStat::new(default_config, default_params))),
        }
    }

    async fn update_loop(&self) {
        let ts = 100;
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(ts));
        loop {
            interval.tick().await;
            let hold_regs = self.holding_registers.lock().unwrap();
            let mut module_stat = self.module_stats.lock().unwrap();
            module_stat.paras.val_a = Self::get_float_from_2_u16(&hold_regs, 2) as f64;
            module_stat.paras.val_b = Self::get_float_from_2_u16(&hold_regs, 4) as f64;
            module_stat.paras.val_out = Self::get_float_from_2_u16(&hold_regs, 6) as f64;

            let out = (module_stat.config.h_liquid * 9.81 * 2.0).sqrt()
                * PI
                * 2.0
                * module_stat.config.r_pipe.powi(2);

            let input: f64 =
                module_stat.paras.val_a + module_stat.paras.val_b - module_stat.paras.val_out * out;
            module_stat.config.h_liquid +=
                ts as f64 / 1000.0 * input / 1000.0 / module_stat.config.a_tank;
            module_stat.config.h_liquid = module_stat
                .config
                .h_liquid
                .max(0.0)
                .min(module_stat.config.h_tank);
            let mut in_regs = self.input_registers.lock().unwrap();
            (in_regs[3], in_regs[2]) = Self::float_to_2_u16_be(module_stat.config.h_liquid as f32);
        }
    }
}

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
