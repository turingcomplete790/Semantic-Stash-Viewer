# Golden operation files

One `.graphql` file per named GraphQL operation the viewer sends to Stash, holding the exact
query text Cynic generates for it. They are the query budget's selection snapshots
(`specs/008-finish-browse-play/contracts/query-budget.md`, research R2): a change to what an
operation selects shows up here, in review, before it reaches Stash.

`crates/stash-core/tests/graphql_operations.rs` builds every operation listed by
`stash_core::adapter::operations::documents()` and compares it with its file here. The same test
checks the tier rules (fields card and chip operations must not select) and the writes rule (only
the play-history and tag-colour operations may be mutations).

After an intended change, regenerate the files and review the diff:

```bash
UPDATE_GOLDEN=1 cargo test -p stash-core --test graphql_operations
```

A new operation needs an entry in `documents()` and a file here; the test fails for any listed
operation without one.
