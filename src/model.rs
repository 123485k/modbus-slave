use std::{
    f64::consts::PI,
    sync::{Arc, Mutex},
};

use tokio::time;

pub struct ModelConfig {
    pub a_tank: f64,
    pub h_tank: f64,
    pub r_pipe: f64,
    pub h_liquid: f64,
}

pub struct ModelParams {
    pub val_a: f64,
    pub val_b: f64,
    pub val_out: f64,
}

pub struct ModelStat {
    pub config: ModelConfig,
    pub paras: ModelParams,
}

impl ModelStat {
    pub fn new(config: ModelConfig, paras: ModelParams) -> Self {
        Self { config, paras }
    }
}

#[derive(Clone)]
pub struct ModleHandler {
    pub input_registers: Arc<Mutex<Vec<u16>>>,
    pub holding_registers: Arc<Mutex<Vec<u16>>>,
    pub module_stats: Arc<Mutex<ModelStat>>,
}

impl ModleHandler {
    pub fn get_float_from_2_u16(v: &[u16], addr: u16) -> f32 {
        let high = v[addr as usize] as u32;
        let low = v[addr as usize + 1] as u32;
        let combined = if cfg!(target_endian = "little") {
            ((low as u32) << 16) | (high as u32)
        } else {
            ((high as u32) << 16) | (low as u32)
        };
        f32::from_bits(combined)
    }

    pub fn float_to_2_u16_be(value: f32) -> (u16, u16) {
        let bits = value.to_bits();
        ((bits >> 16) as u16, bits as u16)
    }

    pub fn new() -> Self {
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

    pub async fn update_loop(&self) {
        let ts = 100;
        let mut interval = time::interval(time::Duration::from_millis(ts));
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
