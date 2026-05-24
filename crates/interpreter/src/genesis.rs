// Genesis seed CAIDs — computed once at genesis freeze using content_hash_v1().
// DO NOT modify. If root_with_system() changes, update these by running:
//   cargo test seed_caids_are_stable -- --nocapture
// and copy the printed "UPDATE:" lines into the constants below.

pub const SEED_MATH:      &str = "hash:sha256:v1:1cacd3c2f58b63cba69d2f58b479e21ecf33311fae89788c78132c0b6c439b45";
pub const SEED_LIST:      &str = "hash:sha256:v1:83be1aaa5e41ff7bba686dccd4846cced091bababc738d7d4dd3b039eec17e5e";
pub const SEED_COND:      &str = "hash:sha256:v1:17afe8ef452181276edc99f124dc0a0acbe59b6cbebecf58d073f0a62a383469";
pub const SEED_DISCOVERY: &str = "hash:sha256:v1:1cfb41b083aeedd1c0acbe2c6a153809006ee30f8af6aa7b12f7aef7cb34d295";
pub const SEED_STRING:    &str = "hash:sha256:v1:95d83dfacfc828d4a4b8812d30e84acd0d605e96098f7505e12d7d4e93b9e101";
pub const SEED_COMPLEX:   &str = "hash:sha256:v1:bb982ffea2042ab09c8f2320a6562244638cf6eb9a5e010503ff86591aef0b65";
pub const SEED_REFL:      &str = "hash:sha256:v1:681c55de94a5d77464c5e048904c1ebedf6e08bdffb8e8c3aa509a6e2db64cf0";
pub const SEED_TIME:      &str = "hash:sha256:v1:a544d35e9fba77b8af42571318a28014a4f0d1f23e1652b350e7254382b05b11";
pub const SEED_TYPE_LIST: &str = "hash:sha256:v1:eb600acdc99e27df1c2420f1d2e6a48d530f19b728a2a7149c36772ee1e71c86";
pub const SEED_BYTES:     &str = "hash:sha256:v1:16f5b680694f6d5dc860ae582d9837bc709f8e911513e9bd634ceeb031495e91";

pub const SEED_OPTION:    &str = "hash:sha256:v1:882e630c8f1cf5cd644ae7cfe6561e8738911c0b5105dcd0680266433400a89d";
pub const SEED_RESULT:    &str = "hash:sha256:v1:bf98c6ee2b26ba36628f5b14050cd48b090e6e4e8248bb8e6e98a9d26fb66c46";
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
        ("~%Bytes",      SEED_BYTES),
        ("@option",      SEED_OPTION),
        ("@result",      SEED_RESULT),
        ("@list",        SEED_TYPE_LIST),
        ("~%Config",     SEED_CONFIG),
    ]
}
