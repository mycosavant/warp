use anyhow::{Context as _, Result};

fn main() -> Result<()> {
    // If this file changes, regenerate the Rust sources.
    println!("cargo:rerun-if-changed=build.rs");
    // Fork (T11.1): and if the *schema* changes, which is the input this script
    // actually reads. Without this line cargo has no reason to re-run the
    // registration, so an edited `schema.graphql` leaves a stale registration in
    // `OUT_DIR` and every `QueryFragment` in this crate is validated against the
    // old schema. It fails in the least helpful way available — "no field
    // `inviteLink` on the GraphQL type `Team`" pointing at Rust that is correct,
    // about a schema on disk that already has the field.
    //
    // Found on 2026-08-24 taking an upstream merge that changed both the queries
    // and the schema together (`5071a868c`, invite-code → invite-link). It stayed
    // hidden through a `cargo check --workspace --all-targets` because this crate
    // was cached, and only appeared once something else forced it to rebuild —
    // so the hazard is specifically *merging*, which this fork now does on a
    // cadence.
    println!("cargo:rerun-if-changed=../warp_graphql_schema/api/schema.graphql");

    // We need to register the schema here, even though the code is generated in the schema crate.
    cynic_codegen::register_schema("warp-server")
        .from_sdl_file("../warp_graphql_schema/api/schema.graphql")
        .context("Should be able to register schema")?
        .as_default()?;

    Ok(())
}
