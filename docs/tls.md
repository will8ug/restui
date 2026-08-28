# TLS Troubleshooting

restui uses rustls and loads the operating system's certificate store by default
(macOS Keychain, Linux system certificate directories), with Mozilla's root list as
a fallback. Certificates your machine already trusts — including corporate or
locally-installed CAs — work without any flags.

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
