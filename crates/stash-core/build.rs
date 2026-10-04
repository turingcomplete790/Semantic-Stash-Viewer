//! Registers Stash v0.31.1's GraphQL schema for Cynic (007 research R5), so every query and
//! fragment in `adapter/` is checked against it at build time.

fn main() {
    println!("cargo:rerun-if-changed=graphql/stash-v0.31.1.graphql");
    cynic_codegen::register_schema("stash")
        .from_sdl_file("graphql/stash-v0.31.1.graphql")
        .expect("Stash schema SDL is readable")
        .as_default()
        .expect("Stash schema registers as the default");
}
