//! A minimal `graphql-transport-ws` client for Stash's `jobsSubscribe` (004 research R5).
//!
//! Connects to `ws(s)://<base>/graphql` with the `ApiKey` header and the profile's TLS mode
//! (Principle VII: same server, same key, no new destinations). Protocol: `connection_init` →
//! `connection_ack` → `subscribe` → `next`…, answering `ping` with `pong`.

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::Connector;

use super::{parse_next, JobUpdate};
use crate::adapter::jobs::JOBS_SUBSCRIBE;
use crate::adapter::StashClient;
use crate::error::AppError;

const PROTOCOL: &str = "graphql-transport-ws";
const ACK_TIMEOUT: Duration = Duration::from_secs(10);

fn internal(message: impl std::fmt::Display) -> AppError {
    AppError::Internal {
        message: format!("jobs subscription: {message}"),
    }
}

/// Subscribe to job updates. Returns a channel that yields updates until the socket closes or
/// the receiver is dropped (which closes the socket).
pub async fn subscribe_jobs(client: &StashClient) -> Result<mpsc::Receiver<JobUpdate>, AppError> {
    let mut url = client.endpoint("graphql");
    let scheme = if url.scheme() == "https" { "wss" } else { "ws" };
    url.set_scheme(scheme)
        .map_err(|()| internal("bad URL scheme"))?;

    let mut request = url.as_str().into_client_request().map_err(internal)?;
    let headers = request.headers_mut();
    headers.insert("Sec-WebSocket-Protocol", HeaderValue::from_static(PROTOCOL));
    if let Some(key) = client.api_key() {
        headers.insert("ApiKey", HeaderValue::from_str(key).map_err(internal)?);
    }

    let connector = (scheme == "wss" && !client.strict_tls())
        .then(|| Connector::Rustls(Arc::new(lenient::client_config())));
    let (mut socket, _) =
        tokio_tungstenite::connect_async_tls_with_config(request, None, false, connector)
            .await
            .map_err(internal)?;

    socket
        .send(Message::text(
            json!({ "type": "connection_init", "payload": {} }).to_string(),
        ))
        .await
        .map_err(internal)?;
    tokio::time::timeout(ACK_TIMEOUT, async {
        while let Some(message) = socket.next().await {
            let Message::Text(text) = message.map_err(internal)? else {
                continue;
            };
            let v: Value = serde_json::from_str(&text).map_err(internal)?;
            match v.get("type").and_then(Value::as_str) {
                Some("connection_ack") => return Ok(()),
                Some("ping") => {
                    socket
                        .send(Message::text(json!({ "type": "pong" }).to_string()))
                        .await
                        .map_err(internal)?;
                }
                _ => {}
            }
        }
        Err(internal("closed before connection_ack"))
    })
    .await
    .map_err(|_| internal("no connection_ack in time"))??;

    socket
        .send(Message::text(
            json!({ "id": "1", "type": "subscribe", "payload": { "query": JOBS_SUBSCRIBE } })
                .to_string(),
        ))
        .await
        .map_err(internal)?;

    let (tx, rx) = mpsc::channel(64);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                () = tx.closed() => break,
                message = socket.next() => {
                    let Some(Ok(message)) = message else { break };
                    let text = match message {
                        Message::Text(text) => text,
                        Message::Close(_) => break,
                        _ => continue,
                    };
                    let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                    match v.get("type").and_then(Value::as_str) {
                        Some("next") => {
                            if let Some(update) = parse_next(&v) {
                                if tx.send(update).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Some("ping") => {
                            let pong = json!({ "type": "pong" }).to_string();
                            if socket.send(Message::text(pong)).await.is_err() {
                                break;
                            }
                        }
                        Some("error" | "complete") => break,
                        _ => {}
                    }
                }
            }
        }
        let _ = socket.close(None).await;
    });
    Ok(rx)
}

/// TLS without certificate checks, for profiles with strict TLS off (the default, Principle
/// VII), matching the HTTP client's `danger_accept_invalid_certs`.
mod lenient {
    use std::sync::Arc;

    use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
    use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
    use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
    use rustls::{DigitallySignedStruct, SignatureScheme};

    #[derive(Debug)]
    struct AcceptAny(Arc<CryptoProvider>);

    impl ServerCertVerifier for AcceptAny {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp_response: &[u8],
            _now: UnixTime,
        ) -> Result<ServerCertVerified, rustls::Error> {
            Ok(ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            verify_tls12_signature(
                message,
                cert,
                dss,
                &self.0.signature_verification_algorithms,
            )
        }

        fn verify_tls13_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            verify_tls13_signature(
                message,
                cert,
                dss,
                &self.0.signature_verification_algorithms,
            )
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            self.0.signature_verification_algorithms.supported_schemes()
        }
    }

    pub(super) fn client_config() -> rustls::ClientConfig {
        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        rustls::ClientConfig::builder_with_provider(Arc::clone(&provider))
            .with_safe_default_protocol_versions()
            .expect("default TLS versions")
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAny(provider)))
            .with_no_client_auth()
    }
}
