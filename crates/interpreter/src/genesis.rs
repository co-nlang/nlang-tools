// Genesis seed CAIDs — computed once at genesis freeze using content_hash_v1().
// DO NOT modify. If root_with_system() changes, update these by running:
//   cargo test seed_caids_are_stable -- --nocapture
// and copy the printed "UPDATE:" lines into the constants below.

pub const SEED_MATH:      &str = "hash:sha256:v1:22a5a2d20c1908c70145ba82e7132f4be7c80ebd45af64986ec0dba1a2618138";
pub const SEED_LIST:      &str = "hash:sha256:v1:45574ba1dbd7b0c0789507c3adcb145625a7d83bfd9a6653a20e245b5ea9fcbb";
pub const SEED_COND:      &str = "hash:sha256:v1:17afe8ef452181276edc99f124dc0a0acbe59b6cbebecf58d073f0a62a383469";
pub const SEED_DISCOVERY: &str = "hash:sha256:v1:8eb6a69c480926cd2acb78bbdcadea1ec33be037688690df2fde0e30e4021ef3";
pub const SEED_STRING:    &str = "hash:sha256:v1:e34106e36d87fdb6e474664cefdf67f7fefb32808b501472a62822c051539cc1";
pub const SEED_COMPLEX:   &str = "hash:sha256:v1:bb982ffea2042ab09c8f2320a6562244638cf6eb9a5e010503ff86591aef0b65";
pub const SEED_REFL:      &str = "hash:sha256:v1:eb2a1045b05c808b8608229c54b297c11f34310e8b833f0d866d28574b0c09a3";
pub const SEED_TIME:      &str = "hash:sha256:v1:c5fe8bb62ac855e7acc6ad95701aa1d0dd21abf616f77b1f8f5760f91739d012";

pub fn all_seeds() -> Vec<(&'static str, &'static str)> {
    vec![
        ("~%Math",       SEED_MATH),
        ("~%List",       SEED_LIST),
        ("~%Cond",       SEED_COND),
        ("~%Discovery",  SEED_DISCOVERY),
        ("~%String",     SEED_STRING),
        ("~%Complex",    SEED_COMPLEX),
        ("~%Reflection", SEED_REFL),
        ("~%Time",       SEED_TIME),
    ]
}
