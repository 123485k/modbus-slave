use crate::model::ModleHandler;
use crate::modbus::ModbusRegisterAccess;
use tokio_modbus::prelude::ExceptionCode;

impl ModbusRegisterAccess for ModleHandler {
    fn read_input_regs(&self, _addr: u16, _cnt: u16) -> Result<Vec<u16>, ExceptionCode> {
        let regs = self.input_registers.lock().unwrap();
        if _addr + _cnt > regs.len() as u16 {
            return Err(ExceptionCode::IllegalDataAddress);
        }
        Ok(regs[_addr as usize..(_addr + _cnt) as usize].to_vec())
    }

    fn read_holding_regs(&self, _addr: u16, _cnt: u16) -> Result<Vec<u16>, ExceptionCode> {
        let regs = self.holding_registers.lock().unwrap();
        if _addr + _cnt > regs.len() as u16 {
            return Err(ExceptionCode::IllegalDataAddress);
        }
        Ok(regs[_addr as usize..(_addr + _cnt) as usize].to_vec())
    }

    fn write_regs(&mut self, _addr: u16, _values: &[u16]) -> Result<(), ExceptionCode> {
        let mut hold_regs = self.holding_registers.lock().unwrap();
        if _addr as usize + _values.len() > hold_regs.len() {
            return Err(ExceptionCode::IllegalDataAddress);
        }
        hold_regs[_addr as usize..(_addr as usize + _values.len())].copy_from_slice(_values);
        Ok(())
    }
}
