# Stash GraphQL schema

`schema.json` is the introspection result from **Stash v0.31.1** (`hash 4de2351e`, appSchema
85), fetched on 2026-09-24 with:

```bash
python3 crates/stash-core/graphql/fetch-schema.py http://localhost:9999
```

`graphql_client` generates typed Rust for each `*.graphql` operation in this directory from this
schema. When the minimum Stash version is raised, re-fetch it and review the diff.
