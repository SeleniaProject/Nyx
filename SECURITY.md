# Security Policy

## Reporting Security Issues

- **Please report vulnerabilities privately** via GitHub Security Advisories
- **Do not open public issues** for security reports
- **Do not disclose security vulnerabilities publicly** until they are fixed
- We aim to acknowledge reports within 72 hours and provide a timeline for fixes

## Supported Versions

| Version | Supported |
|---------|-----------|
| Latest   | ✅        |
| All others | ❌      |

## Security Features

Nyx Protocol implements several security measures:

### Cryptographic Security
- **Post-Quantum Ready**: Hybrid X25519 + Kyber1024 handshakes
- **AEAD Encryption**: ChaCha20-Poly1305 for authenticated encryption
- **Forward Secrecy**: HKDF-based key derivation with perfect forward secrecy
- **Replay Protection**: Anti-replay windows and sequence number validation

### Network Security
- **Traffic Analysis Protection**: Cover traffic and padding
- **NAT Traversal**: Secure ICE/STUN implementation
- **Connection Security**: TLS 1.3 equivalent security through Noise Protocol
- **Access Control**: Token-based authentication for daemon RPC

### Implementation Security
- **Memory Safety**: Rust's ownership system prevents common vulnerabilities
- **No Unsafe Code**: Complete avoidance of unsafe Rust code
- **Constant Time**: Cryptographic operations use constant-time algorithms
- **Zeroization**: Sensitive data is properly zeroized after use

## Security Best Practices

### For Developers
- Always use the latest stable Rust compiler
- Enable all security-related compiler flags
- Review code for timing attacks and side-channel vulnerabilities
- Implement proper error handling without information leakage

### For Operators
- Run Nyx behind a firewall and use proper network segmentation
- Use strong, randomly generated authentication tokens
- Keep systems updated with latest security patches
- Monitor logs for suspicious activity
- Use TLS termination proxies when exposing services

## Vulnerability Disclosure

1. **Report** the vulnerability via GitHub Security Advisories
2. **Wait** for acknowledgment and initial assessment
3. **Cooperate** on providing additional information if requested
4. **Wait** for the fix to be developed and tested
5. **Receive** notification when the fix is ready
6. **Do not disclose** the vulnerability until the fix is publicly available

## Bug Bounty

Currently, we do not offer a formal bug bounty program. However, significant security contributions may be eligible for recognition or other forms of compensation at the maintainers' discretion.

## Contact

For security-related inquiries, please use GitHub Security Advisories rather than email or other communication channels to ensure proper tracking and response.
