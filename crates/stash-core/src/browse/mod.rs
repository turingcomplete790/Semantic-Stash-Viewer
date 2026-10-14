//! The list layer every library section shares (012 research R1): the query (search, criteria,
//! sort, direction, seed), paging, and the entity each list shows. Scenes and Images are
//! instances; their own parts (Scenes' queue and playback, Images' viewer) live beside them.

pub mod entity;
pub mod paging;
pub mod query;
