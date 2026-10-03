//! A fresh DTLS certificate per connection in libwebrtc's shape (as the vanilla client sends):
//! `CN=WebRTC`, ECDSA P-256/SHA-256, a 64-bit serial with the top bit set, valid from a day ago for
//! 31 days, no extensions. Never shared between connections, so bots can't be linked by it.

use rand_core::{OsRng, RngCore};
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, SerialNumber, PKCS_ECDSA_P256_SHA256};
use str0m::config::DtlsCert;
use time::{Duration, OffsetDateTime};

use crate::Error;

pub(crate) fn libwebrtc_certificate() -> Result<DtlsCert, Error> {
    let fail = |e: rcgen::Error| Error::Rtc(format!("certificate: {e}"));
    let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).map_err(fail)?;
    let mut params = CertificateParams::new(Vec::<String>::new()).map_err(fail)?;
    let mut name = DistinguishedName::new();
    name.push(DnType::CommonName, "WebRTC");
    params.distinguished_name = name;
    let now = OffsetDateTime::now_utc();
    params.not_before = now - Duration::days(1);
    params.not_after = now + Duration::days(30);
    let mut serial = OsRng.next_u64().to_be_bytes();
    serial[0] |= 0x80;
    params.serial_number = Some(SerialNumber::from(serial.to_vec()));
    let cert = params.self_signed(&key).map_err(fail)?;
    Ok(DtlsCert { certificate: cert.der().to_vec(), private_key: key.serialize_der() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_libwebrtc_shape_and_is_unique() {
        let (a, b) = (libwebrtc_certificate().unwrap(), libwebrtc_certificate().unwrap());
        assert_ne!(a.certificate, b.certificate);
        let der = &a.certificate;
        // CN=WebRTC, and no X.509 extensions (no OID 2.5.29.x).
        assert!(der.windows(6).any(|w| w == b"WebRTC"));
        assert!(!der.windows(2).any(|w| w == [0x55, 0x1d]));
        // Serial: DER INTEGER of 9 bytes (leading zero, then 8 with the top bit set).
        let serial = der.windows(3).position(|w| w == [0x02, 0x09, 0x00]).expect("9-byte serial");
        assert!(der[serial + 3] & 0x80 != 0);
    }
}
