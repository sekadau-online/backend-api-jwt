use axum::{http::Request, middleware::Next, response::Response};
use std::net::IpAddr;
use std::str::FromStr;
use ipnet::IpNet;
use once_cell::sync::Lazy;
use std::sync::RwLock;

/// Extension type to store resolved client IP
#[derive(Clone, Debug)]
pub struct ClientIp(pub IpAddr);

static TRUSTED_PROXES: Lazy<RwLock<Vec<IpNet>>> = Lazy::new(|| RwLock::new(parse_trusted_proxies()));

fn parse_trusted_proxies() -> Vec<IpNet> {
    let raw = std::env::var("TRUSTED_PROXIES").unwrap_or("".to_string());
    raw.split(',')
        .filter_map(|s| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                match IpNet::from_str(t) {
                    Ok(net) => Some(net),
                    Err(_) => None,
                }
            }
        })
        .collect()
}

fn peer_is_trusted(peer: Option<&std::net::SocketAddr>) -> bool {
    // If no proxies configured, don't trust by default
    match TRUSTED_PROXES.read() {
        Ok(proxies) => {
            if proxies.is_empty() {
                return false;
            }
            match peer {
                Some(sa) => proxies.iter().any(|net| net.contains(&sa.ip())),
                None => false,
            }
        }
        Err(e) => {
            tracing::warn!("proxy_middleware: TRUSTED_PROXIES lock poisoned: {}", e);
            false
        }
    }
}

pub async fn proxy_middleware(mut req: Request<axum::body::Body>, next: Next) -> Response {
    // Try to determine client IP using headers when the immediate peer is a trusted proxy
    let peer = req.extensions().get::<std::net::SocketAddr>().cloned();
    let trusted = peer_is_trusted(peer.as_ref());

    // Prefer Cloudflare header, then X-Forwarded-For, then X-Real-IP
    let maybe_ip = if trusted {
        if let Some(cf) = req.headers().get("cf-connecting-ip").and_then(|v| v.to_str().ok()) {
            IpAddr::from_str(cf).ok()
        } else if let Some(xff) = req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            xff.split(',').next().and_then(|s| IpAddr::from_str(s.trim()).ok())
        } else if let Some(xri) = req.headers().get("x-real-ip").and_then(|v| v.to_str().ok()) {
            IpAddr::from_str(xri).ok()
        } else {
            None
        }
    } else {
        None
    };

    let client_ip = maybe_ip.or_else(|| peer.map(|sa| sa.ip()));

    if let Some(ip) = client_ip {
        req.extensions_mut().insert(ClientIp(ip));
    }

    next.run(req).await
}
