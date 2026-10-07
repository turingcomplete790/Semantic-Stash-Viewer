//! The semantic vocabulary request: per-type flag filters, common variants, colour-rule matches
//! (spec FR-003c, FR-003e; research R12; contracts/query-budget.md `SemanticVocabulary`).

use super::StashClient;
use crate::error::AppError;
use crate::semantic::{build_vocabulary, vocabulary_query, ColourRule, SemanticVocabulary};

/// One `SemanticVocabulary` request for `rules` (cached per server as `semantic:vocabulary`).
pub async fn semantic_vocabulary(
    client: &StashClient,
    rules: &[ColourRule],
) -> Result<SemanticVocabulary, AppError> {
    let data = client
        .graphql_document("SemanticVocabulary", vocabulary_query(rules))
        .await?;
    Ok(build_vocabulary(&data, rules))
}
