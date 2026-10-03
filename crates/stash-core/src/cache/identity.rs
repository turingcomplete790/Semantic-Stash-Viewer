//! Server identity (003 research R2).
//!
//! Tells whether the server behind a profile is the one that filled its cache: the same address
//! can later point at a different Stash (a replaced server, a test instance on the same port).
//! It's a 64-bit FNV-1a hash of the normalised address and Stash's database and config paths, so
//! the cache never stores the paths themselves.

use url::Url;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv1a(bytes: &[u8], mut hash: u64) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// 64-bit FNV-1a of `bytes` as 16 lower-case hex digits (also used for cache keys, 005 R7).
pub(crate) fn hash_hex(bytes: &[u8]) -> String {
    format!("{:016x}", fnv1a(bytes, FNV_OFFSET))
}

/// 16 lower-case hex digits identifying one Stash instance at one address.
pub fn server_identity(
    url: &Url,
    database_path: Option<&str>,
    config_path: Option<&str>,
) -> String {
    // `Url` already lower-cases the scheme and host; drop a trailing slash so `…/` == `…`.
    let address = url.as_str().trim_end_matches('/');
    let mut hash = fnv1a(address.as_bytes(), FNV_OFFSET);
    hash = fnv1a(b"\0", hash);
    hash = fnv1a(database_path.unwrap_or("").as_bytes(), hash);
    hash = fnv1a(b"\0", hash);
    hash = fnv1a(config_path.unwrap_or("").as_bytes(), hash);
    format!("{hash:016x}")
}
