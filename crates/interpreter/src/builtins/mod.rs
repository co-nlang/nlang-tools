mod bytes;
mod cond;
mod csv;
mod diff;
mod disc;
mod engine;
mod env;
/// Language → store trust boundary (SPEC_08 §6.3).
pub mod fs_guard;
mod io;
mod json;
mod list;
mod math;
mod path;
mod process;
mod query;
mod reflection;
mod regex;
mod set;
mod stat;
mod string;
mod time;
mod toml;
mod url;

use crate::BuiltinFn;
use std::collections::HashMap;
use std::sync::Arc;

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
    bytes::register_bytes_builtins(&mut m);
    regex::register_regex_builtins(&mut m);
    json::register_json_builtins(&mut m);
    io::register_io_builtins(&mut m);
    env::register_env_builtins(&mut m);
    process::register_process_builtins(&mut m);
    path::register_path_builtins(&mut m);
    query::register_query_builtins(&mut m);
    diff::register_diff_builtins(&mut m);
    set::register_set_builtins(&mut m);
    stat::register_stat_builtins(&mut m);
    csv::register_csv_builtins(&mut m);
    url::register_url_builtins(&mut m);
    toml::register_toml_builtins(&mut m);

    m
}
