//! TLS behaviour against a local HTTPS server with a self-signed certificate (FR-018, FR-019).
//!
//! wiremock has no TLS support, so this uses a minimal tokio-rustls responder that answers
//! `/healthz` and `/graphql` like Stash v0.31.1.

use std::net::SocketAddr;
use std::sync::Arc;

use stash_core::connection::connect::{test_connection, ConnectOptions};
use stash_core::connection::security::SecurityState;
use stash_core::connection::ConnectFailure;
use stash_core::profiles::ProfileDraft;
use stash_core::AppError;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PROBE_OK: &str = include_str!("fixtures/stash-v0.31.1/probe-ok.json");

/// Start an HTTPS server with a fresh self-signed certificate for `localhost`/`127.0.0.1`.
async fn self_signed_stash() -> SocketAddr {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into(), "127.0.0.1".into()])
        .expect("self-signed cert");
    let cert_der = CertificateDer::from(cert.cert.der().to_vec());
    let key_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der()));
    let config = ServerConfig::builder_with_provider(Arc::new(
        tokio_rustls::rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("protocol versions")
    .with_no_client_auth()
    .with_single_cert(vec![cert_der], key_der)
    .expect("server config");
    let acceptor = TlsAcceptor::from(Arc::new(config));

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        loop {
            let Ok((tcp, _)) = listener.accept().await else {
                return;
            };
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                // A failed handshake (strict client rejecting the cert) just ends here.
                let Ok(mut tls) = acceptor.accept(tcp).await else {
                    return;
                };
                let mut buf = vec![0u8; 8192];
                let n = tls.read(&mut buf).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]);
                let (status, content_type, body) = if request.starts_with("GET /healthz") {
                    ("200 OK", "text/plain", ".")
                } else if request.starts_with("POST /graphql") {
                    ("200 OK", "application/json", PROBE_OK)
                } else {
                    ("404 Not Found", "text/plain", "not found")
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = tls.write_all(response.as_bytes()).await;
                let _ = tls.shutdown().await;
            });
        }
    });
    addr
}

fn draft(address: String, strict_tls: bool) -> ProfileDraft {
    ProfileDraft {
        address,
        strict_tls,
        ..Default::default()
    }
}

#[tokio::test]
async fn self_signed_with_strict_off_connects_unverified() {
    let addr = self_signed_stash().await;
    let outcome = test_connection(
        &draft(format!("https://{addr}"), false),
        &CancellationToken::new(),
        ConnectOptions::default(),
    )
    .await
    .expect("connects with verification off");
    assert_eq!(outcome.security, SecurityState::EncryptedUnverified);
    assert_eq!(outcome.server.counts.scenes, 27552);
}

#[tokio::test]
async fn self_signed_with_strict_on_is_certificate_not_verified() {
    let addr = self_signed_stash().await;
    let err = test_connection(
        &draft(format!("https://{addr}"), true),
        &CancellationToken::new(),
        ConnectOptions::default(),
    )
    .await
    .expect_err("strict must refuse a self-signed cert");
    assert_eq!(
        err,
        AppError::Connect {
            failure: ConnectFailure::CertificateNotVerified
        }
    );
}

#[tokio::test]
async fn strict_scheme_less_address_is_not_downgraded_to_http() {
    // `127.0.0.1:PORT` tries https first; the cert failure must end the search, not fall back.
    let addr = self_signed_stash().await;
    let err = test_connection(
        &draft(addr.to_string(), true),
        &CancellationToken::new(),
        ConnectOptions::default(),
    )
    .await
    .expect_err("strict must refuse");
    assert_eq!(
        err,
        AppError::Connect {
            failure: ConnectFailure::CertificateNotVerified
        }
    );
}

#[tokio::test]
async fn http_to_https_redirect_reports_the_final_scheme() {
    let tls_addr = self_signed_stash().await;
    let plain = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/healthz"))
        .respond_with(
            ResponseTemplate::new(301)
                .insert_header("Location", format!("https://{tls_addr}/healthz").as_str()),
        )
        .mount(&plain)
        .await;

    let outcome = test_connection(
        &draft(plain.uri(), false),
        &CancellationToken::new(),
        ConnectOptions::default(),
    )
    .await
    .expect("follows the redirect to https");
    assert_eq!(
        outcome.base_url.as_str().trim_end_matches('/'),
        format!("https://{tls_addr}")
    );
    assert_eq!(outcome.security, SecurityState::EncryptedUnverified);
}
