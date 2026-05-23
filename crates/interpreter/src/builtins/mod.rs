mod math;
mod cond;
mod string;
mod list;
mod disc;
mod reflection;
mod engine;
mod time;

use std::collections::HashMap;
use std::sync::Arc;
use crate::BuiltinFn;

pub fn create_default_builtins() -> HashMap<String, Arc<BuiltinFn>> {
    let mut m = HashMap::new();
    
    math::register_math_builtins(&mut m);
    math::register_complex_builtins(&mut m);
    cond::register_cond_builtins(&mut m);
    string::register_string_builtins(&mut m);
    list::register_list_builtins(&mut m);
    disc::register_disc_builtins(&mut m);
    reflection::register_reflection_builtins(&mut m);
    engine::register_engine_builtins(&mut m);
    time::register_time_builtins(&mut m);
    
    m
}