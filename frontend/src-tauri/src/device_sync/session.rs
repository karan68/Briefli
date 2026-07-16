use std::net::IpAddr;

use base64::{
    engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD},
    Engine,
};
use chrono::{DateTime, Duration, Utc};
use qrcode::{render::svg, QrCode};
use rand::{rngs::OsRng, RngCore};
use rcgen::{generate_simple_self_signed, CertifiedKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;

use super::protocol::PROTOCOL_VERSION;

const PAIRING_SESSION_MINUTES: i64 = 5;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PairingPayload {
    pub protocol_version: u16,
    pub endpoint: String,
    pub certificate_sha256: String,
    pub pairing_token: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingSessionView {
    pub endpoint: String,
    pub certificate_sha256: String,
    pub expires_at: String,
    pub qr_svg: String,
}

pub struct SessionMaterials {
    pub certificate_der: Vec<u8>,
    pub private_key_der: Vec<u8>,
    pub pairing: PairingSession,
    pub view: PairingSessionView,
    #[cfg(test)]
    pub(crate) pairing_token: String,
}

pub struct PairingSession {
    token_hash: [u8; 32],
    expires_at: DateTime<Utc>,
    consumed: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PairingSessionError {
    #[error("failed to generate the session certificate")]
    CertificateGeneration,
    #[error("failed to serialize the pairing payload")]
    PayloadSerialization,
    #[error("failed to encode the pairing QR code")]
    QrEncoding,
    #[error("pairing session has expired")]
    Expired,
    #[error("pairing session was already used")]
    AlreadyConsumed,
    #[error("pairing token is invalid")]
    InvalidToken,
}

impl PairingSession {
    pub fn verify_and_consume(
        &mut self,
        token: &str,
        now: DateTime<Utc>,
    ) -> Result<(), PairingSessionError> {
        if now > self.expires_at {
            return Err(PairingSessionError::Expired);
        }
        if self.consumed {
            return Err(PairingSessionError::AlreadyConsumed);
        }

        let candidate = Sha256::digest(token.as_bytes());
        if self.token_hash.ct_eq(candidate.as_slice()).unwrap_u8() != 1 {
            return Err(PairingSessionError::InvalidToken);
        }

        self.consumed = true;
        Ok(())
    }
}

pub fn generate_pairing_session(
    local_ip: IpAddr,
    port: u16,
    now: DateTime<Utc>,
) -> Result<SessionMaterials, PairingSessionError> {
    let CertifiedKey { cert, key_pair } =
        generate_simple_self_signed(vec![local_ip.to_string()])
            .map_err(|_| PairingSessionError::CertificateGeneration)?;
    let certificate_der = cert.der().to_vec();
    let private_key_der = key_pair.serialize_der();
    let certificate_sha256 = format!("{:x}", Sha256::digest(&certificate_der));

    let mut token_bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut token_bytes);
    let pairing_token = URL_SAFE_NO_PAD.encode(token_bytes);
    let token_hash: [u8; 32] = Sha256::digest(pairing_token.as_bytes()).into();
    let expires_at = now + Duration::minutes(PAIRING_SESSION_MINUTES);
    let endpoint = format!("https://{local_ip}:{port}");

    let payload = PairingPayload {
        protocol_version: PROTOCOL_VERSION,
        endpoint: endpoint.clone(),
        certificate_sha256: certificate_sha256.clone(),
        pairing_token,
        expires_at: expires_at.to_rfc3339(),
    };
    let encoded_payload = serde_json::to_vec(&payload)
        .map_err(|_| PairingSessionError::PayloadSerialization)?;
    let code = QrCode::new(encoded_payload).map_err(|_| PairingSessionError::QrEncoding)?;
    let qr_svg = code
        .render::<svg::Color>()
        .min_dimensions(320, 320)
        .dark_color(svg::Color("#1f2421"))
        .light_color(svg::Color("#ffffff"))
        .build();

    Ok(SessionMaterials {
        certificate_der,
        private_key_der,
        pairing: PairingSession {
            token_hash,
            expires_at,
            consumed: false,
        },
        view: PairingSessionView {
            endpoint,
            certificate_sha256,
            expires_at: expires_at.to_rfc3339(),
            qr_svg: BASE64_STANDARD.encode(qr_svg.as_bytes()),
        },
        #[cfg(test)]
        pairing_token: payload.pairing_token,
    })
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::*;

    #[test]
    fn creates_pinned_tls_material_and_scannable_payload() {
        let now = DateTime::parse_from_rfc3339("2026-07-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let materials = generate_pairing_session(
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)),
            43111,
            now,
        )
        .unwrap();

        assert!(!materials.certificate_der.is_empty());
        assert!(!materials.private_key_der.is_empty());
        assert_eq!(materials.view.endpoint, "https://192.168.1.20:43111");
        assert_eq!(materials.view.certificate_sha256.len(), 64);
        let svg = BASE64_STANDARD.decode(&materials.view.qr_svg).unwrap();
        assert!(String::from_utf8(svg).unwrap().starts_with("<?xml"));
    }

    #[test]
    fn token_is_valid_once_before_expiry() {
        let now = Utc::now();
        let mut materials = generate_pairing_session(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            43111,
            now,
        )
        .unwrap();

        let token = materials.pairing_token.clone();
        assert_eq!(materials.pairing.verify_and_consume(&token, now), Ok(()));
        assert_eq!(
            materials.pairing.verify_and_consume(&token, now),
            Err(PairingSessionError::AlreadyConsumed)
        );
    }

    #[test]
    fn rejects_wrong_or_expired_token() {
        let now = Utc::now();
        let mut wrong_token_materials = generate_pairing_session(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            43111,
            now,
        )
        .unwrap();
        assert_eq!(
            wrong_token_materials
                .pairing
                .verify_and_consume("wrong", now),
            Err(PairingSessionError::InvalidToken)
        );

        let mut expired_materials = generate_pairing_session(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            43111,
            now,
        )
        .unwrap();
        let token = expired_materials.pairing_token.clone();
        assert_eq!(
            expired_materials
                .pairing
                .verify_and_consume(&token, now + Duration::minutes(6)),
            Err(PairingSessionError::Expired)
        );
    }

}