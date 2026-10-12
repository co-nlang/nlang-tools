// Genesis seed CAIDs — computed once at genesis freeze using content_hash_v1().
// DO NOT modify. If root_with_system() changes, update these by running:
//   cargo test seed_caids_are_stable -- --nocapture
// and copy the printed "UPDATE:" lines into the constants below.
// Q-083／D106 紀元: the digest moved; the v1 spelling did not.

pub const SEED_MATH: &str =
    "hash:sha256:v1:bf43879d0fb609d544e9f5ec5719fe4a5499af3a91a135781caf8601f78d57ba";
pub const SEED_LIST: &str =
    "hash:sha256:v1:a345a4f48e31184a11dcae92e47ccd64ba23fe14632a65244ef9a7c9e6373ff7";
pub const SEED_COND: &str =
    "hash:sha256:v1:422e9fd4ed512fe5529a5476a9c956674dabd542d3f1283d93daff34b58c8b2d";
pub const SEED_DISCOVERY: &str =
    "hash:sha256:v1:6b7639ea7d1592442bbf411fc779d8d456792c62108cc4a1cc888e4a1f8d28d2";
pub const SEED_STRING: &str =
    "hash:sha256:v1:ccd6c9591797988143af3e510bf6ef5ec2623d3303ccd80a00ba0df69b2669f9";
pub const SEED_COMPLEX: &str =
    "hash:sha256:v1:4a37ccbe770069097daabc50b7eda30e950eefcb50621881bd218d6070835b72";
pub const SEED_REFL: &str =
    "hash:sha256:v1:3c74c84145ec1eb8022ef0dec22199ac512852df2648794eaebd89aa3d0d1b78";
pub const SEED_TIME: &str =
    "hash:sha256:v1:054a00b89658f3e9365d814e8dae64cf10ed9c666cfa60127adb914b14e1ab44";
pub const SEED_TYPE_LIST: &str =
    "hash:sha256:v1:2025c9138a2d0ade3c0d96ca8fc83878136e3821d1adac5232f2832d6e474592";
pub const SEED_BYTES: &str =
    "hash:sha256:v1:a833ee1f89b856a8f71f1bed15616e0448a00931309eb9b69aee47ae0e99cc9f";
pub const SEED_JSON: &str =
    "hash:sha256:v1:f41f0d6b90eda14bcc9199e80e0190413fbdcc9d88cca85b35dbb5edd454b2dc";
pub const SEED_IO: &str =
    "hash:sha256:v1:54c31301ea67460a6449f0ada5e2b8b7779eb316c1f44f227251e2a0d2efe8a3";
pub const SEED_ENV: &str =
    "hash:sha256:v1:12317689badc78e6e0e17ff1293cad1ba3314c55a4bf60b18584ea0ce052fd85";
pub const SEED_PROCESS: &str =
    "hash:sha256:v1:cb4c9b709930e8a5ccc229c2ba56cc248a11831b9bd4336964934e8df9abcd62";
pub const SEED_PATH: &str =
    "hash:sha256:v1:0fe2bdb023937798a6e78fc7a007f6c6cb0abbb70d98f5d4e003a676a5353c18";
pub const SEED_QUERY: &str =
    "hash:sha256:v1:bb0a1056a83f2bb0145f4f5960a7bd78f9325f3d38ad9a9449d790664ea578ad";
pub const SEED_DIFF: &str =
    "hash:sha256:v1:fd4e903a5d84989ea32da66eb06931b628c2ce5ee9a282dafe7ce542ae0776ac";
pub const SEED_SET: &str =
    "hash:sha256:v1:f3b6ac51148f41d07fae7bd2422b253a7de8ceea4a4063a55d0de9aff250e088";
pub const SEED_STAT: &str =
    "hash:sha256:v1:71161f99f012c27886a5f5fb27d8cd95f3684a219a2c960c823f18aa6b6d677e";
pub const SEED_CSV: &str =
    "hash:sha256:v1:7622434dd851c7af967678254be07eff477a3c9e46aa04a4e3b83753c26d9b42";
pub const SEED_URL: &str =
    "hash:sha256:v1:d7deb52f872ae34d492374bf5f5937af5671829a841f2912647203aa75fa1329";
pub const SEED_TOML: &str =
    "hash:sha256:v1:b729c0c2dfcd1fe4f2a6e675cc1c97cc90bd2be52b8a53d63b6a3fd25fc097e6";

pub const SEED_OPTION: &str =
    "hash:sha256:v1:9de25c0b68fe5015d4bb02bf69ddad4bf45276dee4a8469f4ee7174479c0b983";
pub const SEED_RESULT: &str =
    "hash:sha256:v1:3a4dd2666e0cc6476f26e25e393fee2b91ab344317ea2ee1dc7c4a3b999a9ba9";
pub const SEED_REGEX: &str =
    "hash:sha256:v1:2ddd433a41e2fdebece8f71e288f7896ffe6b9117b9dcd54b3e85425aa421e3c";
pub const SEED_CONFIG: &str =
    "hash:sha256:v1:17cab1749ad8f1c4bfa881df7d9b3aba3fb584d4c799f5b8823bc18b342984f9";

pub fn all_seeds() -> Vec<(&'static str, &'static str)> {
    vec![
        ("~%Math", SEED_MATH),
        ("~%List", SEED_LIST),
        ("~%Cond", SEED_COND),
        ("~%Discovery", SEED_DISCOVERY),
        ("~%Str", SEED_STRING),
        ("~%Complex", SEED_COMPLEX),
        ("~%Reflection", SEED_REFL),
        ("~%Time", SEED_TIME),
        ("~%Bytes", SEED_BYTES),
        ("~%Json", SEED_JSON),
        ("~%Io", SEED_IO),
        ("~%Env", SEED_ENV),
        ("~%Process", SEED_PROCESS),
        ("~%Path", SEED_PATH),
        ("~%Query", SEED_QUERY),
        ("~%Diff", SEED_DIFF),
        ("~%Set", SEED_SET),
        ("~%Stat", SEED_STAT),
        ("~%Csv", SEED_CSV),
        ("~%Url", SEED_URL),
        ("~%Toml", SEED_TOML),
        ("~%Regex", SEED_REGEX),
        ("@option", SEED_OPTION),
        ("@result", SEED_RESULT),
        ("@list", SEED_TYPE_LIST),
        ("~%Config", SEED_CONFIG),
    ]
}
