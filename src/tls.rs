use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error as TlsError, SignatureScheme};
use rustls_platform_verifier::BuilderVerifierExt;

pub fn client_config(no_verify: bool) -> Result<ClientConfig, TlsError> {
    // rustls-platform-verifier >= 0.6 does not install a process-default
    // provider, so an explicit one is required or the builder panics.
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()?;

    let builder = if no_verify {
        builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier::new(provider)))
    } else {
        builder.with_platform_verifier()?
    };

    let mut config = builder.with_no_client_auth();
    // reqwest's BuiltRustls path skips setting ALPN, so HTTP/2 would silently
    // be lost without this.
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(config)
}

#[derive(Debug)]
struct NoVerifier {
    provider: Arc<CryptoProvider>,
}

impl NoVerifier {
    fn new(provider: Arc<CryptoProvider>) -> Self {
        Self { provider }
    }
}

impl ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_config_with_platform_verifier_builds() {
        let config = client_config(false).unwrap();

        assert_eq!(
            config.alpn_protocols,
            vec![b"h2".to_vec(), b"http/1.1".to_vec()]
        );
    }

    #[test]
    fn client_config_with_no_verify_builds() {
        let config = client_config(true).unwrap();

        assert_eq!(
            config.alpn_protocols,
            vec![b"h2".to_vec(), b"http/1.1".to_vec()]
        );
    }

    #[test]
    fn no_verifier_accepts_any_certificate() {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let verifier = NoVerifier::new(provider);
        let garbage = CertificateDer::from(&b"not a real certificate"[..]);
        let server_name = ServerName::try_from("example.com").unwrap();

        let result = verifier.verify_server_cert(&garbage, &[], &server_name, &[], UnixTime::now());

        assert!(result.is_ok());
    }

    // reqwest downcasts the preconfigured TLS value through `Any`; a rustls
    // version skew between restui and reqwest makes the downcast miss and
    // surfaces only as a build error at runtime.
    #[test]
    fn preconfigured_tls_downcast_succeeds() {
        for no_verify in [false, true] {
            let client = reqwest::blocking::Client::builder()
                .use_preconfigured_tls(client_config(no_verify).unwrap())
                .build();

            assert!(client.is_ok(), "no_verify={no_verify}: {:?}", client.err());
        }
    }

    #[test]
    fn platform_verifier_rejects_garbage_certificate() {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let verifier = rustls_platform_verifier::Verifier::new(provider).unwrap();
        let garbage = CertificateDer::from(&b"not a real certificate"[..]);
        let server_name = ServerName::try_from("example.com").unwrap();

        let result = verifier.verify_server_cert(&garbage, &[], &server_name, &[], UnixTime::now());

        assert!(result.is_err());
    }
}
