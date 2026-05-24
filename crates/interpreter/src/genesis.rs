// Genesis seed CAIDs — computed once at genesis freeze using content_hash_v1().
// DO NOT modify. If root_with_system() changes, update these by running:
//   cargo test seed_caids_are_stable -- --nocapture
// and copy the printed "UPDATE:" lines into the constants below.

pub const SEED_MATH:      &str = "hash:sha256:v1:22a5a2d20c1908c70145ba82e7132f4be7c80ebd45af64986ec0dba1a2618138";
pub const SEED_LIST:      &str = "hash:sha256:v1:45574ba1dbd7b0c0789507c3adcb145625a7d83bfd9a6653a20e245b5ea9fcbb";
pub const SEED_COND:      &str = "hash:sha256:v1:17afe8ef452181276edc99f124dc0a0acbe59b6cbebecf58d073f0a62a383469";
pub const SEED_DISCOVERY: &str = "hash:sha256:v1:1cfb41b083aeedd1c0acbe2c6a153809006ee30f8af6aa7b12f7aef7cb34d295";
pub const SEED_STRING:    &str = "hash:sha256:v1:e34106e36d87fdb6e474664cefdf67f7fefb32808b501472a62822c051539cc1";
pub const SEED_COMPLEX:   &str = "hash:sha256:v1:bb982ffea2042ab09c8f2320a6562244638cf6eb9a5e010503ff86591aef0b65";
pub const SEED_REFL:      &str = "hash:sha256:v1:681c55de94a5d77464c5e048904c1ebedf6e08bdffb8e8c3aa509a6e2db64cf0";
pub const SEED_TIME:      &str = "hash:sha256:v1:c5fe8bb62ac855e7acc6ad95701aa1d0dd21abf616f77b1f8f5760f91739d012";
pub const SEED_OPTION:    &str = "hash:sha256:v1:bcc705a1a6ea76ec70f9a84529ddb5af2bfad99fcac770391d55a217c4abc836";
pub const SEED_RESULT:    &str = "hash:sha256:v1:ed0f387a2fcaeb58f6d43ca5618f17ee7d1839e13deb5b1bcd8113ad6966bc43";
pub const SEED_CONFIG:    &str = "hash:sha256:v1:87afeec4f3c5a2f384733301885ea6497eef5b352f6961576f5c538a05550f73";

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
        ("@option",      SEED_OPTION),
        ("@result",      SEED_RESULT),
        ("~%Config",     SEED_CONFIG),
    ]
}
