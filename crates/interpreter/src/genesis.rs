// Genesis seed CAIDs — computed once at genesis freeze using content_hash_v1().
// DO NOT modify. If root_with_system() changes, update these by running:
//   cargo test seed_caids_are_stable -- --nocapture
// and copy the printed "UPDATE:" lines into the constants below.

pub const SEED_MATH:      &str = "hash:sha256:v1:448343e267bcabb338566ac5b48be74c1f3aa8b6a478aa4bb3f42c8dfc8b5249";
pub const SEED_LIST:      &str = "hash:sha256:v1:881454d725f6c0df8b21e1ac27744185a171e7a4c0c36acbef17aabe677f0686";
pub const SEED_COND:      &str = "hash:sha256:v1:17afe8ef452181276edc99f124dc0a0acbe59b6cbebecf58d073f0a62a383469";
pub const SEED_DISCOVERY: &str = "hash:sha256:v1:1cfb41b083aeedd1c0acbe2c6a153809006ee30f8af6aa7b12f7aef7cb34d295";
pub const SEED_STRING:    &str = "hash:sha256:v1:525dcad5ac90bbb6b01a86b70b82f9897f0501bee64be69ad07ed72bcc9cb400";
pub const SEED_COMPLEX:   &str = "hash:sha256:v1:bb982ffea2042ab09c8f2320a6562244638cf6eb9a5e010503ff86591aef0b65";
pub const SEED_REFL:      &str = "hash:sha256:v1:681c55de94a5d77464c5e048904c1ebedf6e08bdffb8e8c3aa509a6e2db64cf0";
pub const SEED_TIME:      &str = "hash:sha256:v1:a544d35e9fba77b8af42571318a28014a4f0d1f23e1652b350e7254382b05b11";
pub const SEED_TYPE_LIST: &str = "hash:sha256:v1:eb600acdc99e27df1c2420f1d2e6a48d530f19b728a2a7149c36772ee1e71c86";
pub const SEED_BYTES:     &str = "hash:sha256:v1:4f6824f38f0de657b90055ceb9a643a4fcb2e2ecda9c4cba46dd736c22ee9121";
pub const SEED_JSON:      &str = "hash:sha256:v1:0b50257b9d1e84637c576b9cd4b1f478429aeaca07893e2ecb67b491d9bb0337";
pub const SEED_IO:        &str = "hash:sha256:v1:e620bfad72ec3142d4ffbb7d37955496d831e47566551a3609df61d1a47f7590";
pub const SEED_ENV:       &str = "hash:sha256:v1:361d79419a1f56a72f923115812360c8aee25a35f47163068223f13acfcef334";
pub const SEED_PROCESS:   &str = "hash:sha256:v1:e2720f7dd95ce94e03f1d33b724d13a16c1075dcb95b794ebe79f29c5cb25ada";
pub const SEED_PATH:      &str = "hash:sha256:v1:d7f80fd2dc1e98b01a782f21266d99c9df8feb548bb92bd525ce4f0b0b50cb65";
pub const SEED_QUERY:     &str = "hash:sha256:v1:ed1e83ba547dd53732d265531fd219627d5e24bd9583f3255b8a255cac173c3c";

pub const SEED_OPTION:    &str = "hash:sha256:v1:882e630c8f1cf5cd644ae7cfe6561e8738911c0b5105dcd0680266433400a89d";
pub const SEED_RESULT:    &str = "hash:sha256:v1:bf98c6ee2b26ba36628f5b14050cd48b090e6e4e8248bb8e6e98a9d26fb66c46";
pub const SEED_REGEX:     &str = "hash:sha256:v1:80d321e07858a76dedca5edf51b0b93913e95c93da5a6e135070c1b6647e05a3";
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
        ("~%Json",       SEED_JSON),
        ("~%Io",         SEED_IO),
        ("~%Env",        SEED_ENV),
        ("~%Process",    SEED_PROCESS),
        ("~%Path",       SEED_PATH),
        ("~%Query",      SEED_QUERY),
        ("~%Regex",      SEED_REGEX),
        ("@option",      SEED_OPTION),
        ("@result",      SEED_RESULT),
        ("@list",        SEED_TYPE_LIST),
        ("~%Config",     SEED_CONFIG),
    ]
}
