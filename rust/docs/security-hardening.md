# Security Hardening Checklist — dsh-rust

## Overview

This document defines security hardening measures for the dsh-rust framework.

## Checklist

### Input Validation

- [x] API key validation (SHA-256 hashed storage)
- [x] Rate limiting (token bucket algorithm)
- [x] Quota enforcement (daily/monthly limits)
- [x] Tool argument validation (JSON Schema)
- [x] Session event validation (closed enum)

### Authentication & Authorization

- [x] API key authentication
- [x] Permission-based access control
- [x] Key expiration support
- [x] Key revocation
- [ ] JWT token support (future)
- [ ] OAuth2 integration (future)

### Sandbox Security

- [x] macOS sandbox-exec (seatbelt)
- [x] Linux Landlock (kernel 5.13+)
- [x] Windows restricted tokens
- [x] Fail-closed on unavailable sandbox
- [ ] Network namespace isolation (future)
- [ ] Filesystem mount namespace (future)

### Dependency Security

- [x] cargo-deny configuration (license, advisories, sources)
- [x] cargo-audit integration
- [x] Dependency count monitoring
- [ ] Automated vulnerability scanning in CI

### Code Security

- [x] No `unwrap()` in library code
- [x] No `unsafe` code (zero unsafe blocks)
- [x] Error propagation via `Result`
- [x] Fail-closed authentication
- [x] Fail-closed sandbox

### Data Security

- [x] API keys stored as SHA-256 hashes
- [x] Session logs are append-only
- [x] No credentials in logs
- [ ] Encryption at rest (future)
- [ ] TLS for inter-node communication (future)

### Network Security

- [x] CORS configuration
- [x] Rate limiting per IP/user
- [ ] Request size limits
- [ ] Timeout enforcement
- [ ] DDoS protection (future)

## Security Audit Script

Run the security audit:

```bash
./scripts/security-audit.sh
```

Checks:
1. cargo-deny (advisories, licenses, sources)
2. cargo-audit (RustSec database)
3. Unsafe code detection
4. Dependency count
5. unwrap()/expect() in non-test code

## Incident Response

1. **Vulnerability discovered**: Update dependency, run `cargo audit fix`
2. **Key compromised**: Revoke key via `AuthManager::revoke()`
3. **Rate limit bypass**: Check logs, update rate limit config
4. **Sandbox escape**: Investigate, update sandbox policy
