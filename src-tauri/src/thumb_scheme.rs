//! The `ssv-thumb://` scheme (005 contract "URI scheme"): grid thumbnails served from the core,
//! so the webview never decodes a full-size screenshot or sees an API key.
//!
//! `ssv-thumb://localhost/<kind>/<id>?v=<version>` → a 480 px JPEG from the profile's view
//! cache, or fetched and prepared now (`stash_core::thumbs`). `kind` is `scene` for now; others
//! answer 404. When there's nothing to show (no profile, no screenshot, an error) it answers with
//! the placeholder, so cards never show a broken image.

use std::sync::{Arc, Mutex};

use stash_core::adapter::scenes::scene_screenshot_bytes;
use stash_core::thumbs::{SourceFetch, ThumbService, PLACEHOLDER};
use stash_core::AppError;
use tauri::http::{Request, Response, StatusCode};
use tauri::{AppHandle, Manager, UriSchemeContext, UriSchemeResponder};
use uuid::Uuid;

use crate::cache_commands::current_profile;
use crate::player_commands::active_client;
use crate::state::AppState;

pub const SCHEME: &str = "ssv-thumb";

/// One thumbnail service per profile, kept so its concurrency limit covers every request.
#[derive(Default)]
pub struct ThumbServices(Mutex<Option<(Uuid, Arc<ThumbService>)>>);

impl ThumbServices {
    fn for_profile(&self, app: &AppHandle, profile: Uuid) -> Option<Arc<ThumbService>> {
        let mut current = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((id, service)) = current.as_ref() {
            if *id == profile {
                return Some(Arc::clone(service));
            }
        }
        let state = app.state::<AppState>();
        let refresher = state.caches.for_profile(profile)?;
        // The client is looked up per fetch, so a reconnect (or a changed key) is picked up.
        let handle = app.clone();
        let fetch: SourceFetch = Arc::new(move |id: String| {
            let handle = handle.clone();
            Box::pin(async move {
                let client = {
                    let state = handle.state::<AppState>();
                    active_client(&state).map(|(client, _, _)| client)
                };
                match client {
                    Ok(client) => scene_screenshot_bytes(&client, &id).await,
                    Err(_) => Err(AppError::NotConnected),
                }
            })
        });
        let service = Arc::new(ThumbService::new(Arc::clone(refresher.cache()), fetch));
        *current = Some((profile, Arc::clone(&service)));
        Some(service)
    }
}

/// `/<kind>/<id>` and `v` from the request URI (any host: Linux uses `ssv-thumb://localhost`,
/// Windows `http://ssv-thumb.localhost`).
fn parse(uri: &tauri::http::Uri) -> Option<(String, String, String)> {
    let mut parts = uri.path().trim_matches('/').split('/');
    let kind = parts.next().filter(|k| !k.is_empty())?.to_owned();
    let id = parts.next().filter(|i| !i.is_empty())?.to_owned();
    if parts.next().is_some()
        || !id.chars().all(|c| c.is_ascii_alphanumeric())
        || !kind.chars().all(|c| c.is_ascii_lowercase())
    {
        return None;
    }
    let version = uri
        .query()
        .and_then(|q| q.split('&').find_map(|pair| pair.strip_prefix("v=")))
        .filter(|v| !v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("0")
        .to_owned();
    Some((kind, id, version))
}

fn image(bytes: Vec<u8>, cache_forever: bool) -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "image/jpeg")
        .header(
            "Cache-Control",
            if cache_forever {
                "max-age=31536000, immutable"
            } else {
                "no-store"
            },
        )
        .body(bytes)
        .unwrap_or_default()
}

fn not_found() -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Vec::new())
        .unwrap_or_default()
}

/// The handler for `register_asynchronous_uri_scheme_protocol`. Work runs on the async runtime, never on
/// the webview's thread.
pub fn handle(
    ctx: UriSchemeContext<'_, tauri::Wry>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = ctx.app_handle().clone();
    let Some((kind, id, version)) = parse(request.uri()) else {
        responder.respond(not_found());
        return;
    };
    tauri::async_runtime::spawn(async move {
        let service = {
            let state = app.state::<AppState>();
            current_profile(&state)
        }
        .and_then(|profile| app.state::<ThumbServices>().for_profile(&app, profile));
        let Some(service) = service else {
            responder.respond(image(PLACEHOLDER.to_vec(), false));
            return;
        };
        match service.get(&kind, &id, &version).await {
            Some(bytes) => {
                let real = bytes.as_slice() != PLACEHOLDER;
                // Only real thumbnails are immutable; a placeholder may become real later.
                responder.respond(image(bytes, real));
            }
            None => responder.respond(not_found()),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::parse;

    fn p(s: &str) -> Option<(String, String, String)> {
        parse(&s.parse().expect("uri"))
    }

    #[test]
    fn parses_kind_id_and_version() {
        assert_eq!(
            p("ssv-thumb://localhost/scene/501?v=1715900000"),
            Some(("scene".into(), "501".into(), "1715900000".into()))
        );
        assert_eq!(
            p("http://ssv-thumb.localhost/scene/7"),
            Some(("scene".into(), "7".into(), "0".into()))
        );
    }

    #[test]
    fn rejects_anything_else() {
        assert_eq!(p("ssv-thumb://localhost/scene"), None);
        assert_eq!(p("ssv-thumb://localhost/scene/1/extra"), None);
        assert_eq!(p("ssv-thumb://localhost/scene/../etc"), None);
        assert_eq!(p("ssv-thumb://localhost/Scene/1"), None);
        // A malformed version is ignored rather than put into a cache key.
        assert_eq!(
            p("ssv-thumb://localhost/scene/1?v=a:b"),
            Some(("scene".into(), "1".into(), "0".into()))
        );
    }
}
