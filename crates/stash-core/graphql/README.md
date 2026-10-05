# Stash GraphQL schema

`stash-v0.31.1.graphql` is Stash **v0.31.1**'s schema (SDL), the constitution's minimum
version. `build.rs` registers it with Cynic, so every query and fragment in `src/adapter/` is
checked against it at build time (007 research R5). It was assembled from the Stash source at tag
`v0.31.1`:

```bash
git -C ~/stash-source/stash show v0.31.1:graphql/schema/schema.graphql > stash-v0.31.1.graphql
for f in $(git -C ~/stash-source/stash ls-tree --name-only v0.31.1 graphql/schema/types/ | sort); do
  git -C ~/stash-source/stash show "v0.31.1:$f" >> stash-v0.31.1.graphql
done
```

When the minimum Stash version is raised, regenerate it from the new tag and review the diff;
the build then fails wherever a query no longer matches.

`schema.json` is the same version's introspection result (fetched with `fetch-schema.py`), kept
for reference. `jobs_subscribe.graphql` is the jobs subscription document, sent as-is by the
hand-written `graphql-transport-ws` client in `src/jobs/ws.rs`.
