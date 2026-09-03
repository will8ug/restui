# TLS Troubleshooting

restui delegates certificate verification to the operating system's native trust
engine:

- **macOS**: `SecTrustEvaluateWithError` (Keychain, including MDM-installed
  configuration profiles)
- **Linux**: system certificate directories, via rustls-native-certs and
  openssl-probe
- **Windows**: CryptoAPI

Certificates the OS trusts, including corporate CAs installed via MDM or
configuration profile and TLS-inspecting proxy roots, are honored without any
flag.

## Common fixes

### 1. Trust your custom CA in the OS

Add the CA certificate to your system store and restui picks it up automatically on
the next start:

- **macOS**: `sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain ca.crt`
- **Debian/Ubuntu**: copy the cert to `/usr/local/share/ca-certificates/` and run `sudo update-ca-certificates`
- **Fedora/RHEL**: copy the cert to `/etc/pki/ca-trust/source/anchors/` and run `sudo update-ca-trust`

### 2. Skip certificate verification (not recommended)

As a last resort, disable TLS verification entirely:

```bash
restui --no-verify api.http
```

**Warning:** this disables all certificate checks, making connections vulnerable to
man-in-the-middle attacks. Use only for local development or trusted networks.

## Platform support

The npm package ships prebuilt binaries for macOS (arm64, x64) and Linux x64 (glibc).
On musl-based Linux, Windows, or other architectures, the `restui` shim exits with a
clear error — install from source instead:

```bash
cargo install --git https://github.com/will8ug/restui
```

## Linux environment variables

On Linux only, `SSL_CERT_FILE` and `SSL_CERT_DIR`, if set in the environment,
override the normal system certificate directory lookup. This can silently
prevent restui from trusting certificates the OS would otherwise trust,
including corporate CAs. Unset them if you hit unexpected certificate errors.
