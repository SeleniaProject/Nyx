# Nyx WASM SDK

WebAssembly bindings for Nyx client operations, providing browser-compatible subset of the Nyx Protocol v1.0.

## Features

### Core Features (Always Available)
- ✅ SDK initialization and configuration management
- ✅ Version information and capability detection
- ✅ Cross-platform compatibility (browser/Node.js/WASI)

### Cryptographic Features
- ✅ **Noise Handshake Demo** (`noise_handshake_demo`) - Cryptographic showcase of Noise protocol
  - Supports XX pattern with Curve25519, ChaCha20Poly1305, BLAKE2s
  - Message exchange simulation with performance metrics
  - Configurable pre-shared keys and static keypairs

### Network Integration
- ✅ **Push Registration** (`nyx_register_push`) - Gateway integration helper
  - VAPID-compatible push endpoint generation
  - Service Worker registration support
  - Gateway configuration with client identification

### Data Plane Features
- ✅ **Multipath Management** - Dynamic path selection and optimization
  - Multiple path monitoring with quality metrics
  - Adaptive bandwidth measurement and jitter calculation
  - Health checking and performance history tracking

### Advanced Cryptography (Optional)
- ✅ **HPKE Support** (feature: `hpke`) - Hybrid Public Key Encryption
  - X25519 + ChaCha20Poly1305 cipher suite
  - Key generation and encryption/decryption operations
  - Base64-encoded ciphertext handling

## Browser Compatibility

| Feature | Status | Notes |
|---------|--------|-------|
| Noise Demo | ✅ Supported | Full handshake simulation |
| Push Registration | ✅ Supported | Service Worker integration |
| Multipath | ✅ Supported | Client-side path management |
| HPKE | ✅ Supported | When feature enabled |
| Plugin System | ⚠️ Limited | Browser sandbox restrictions |
| Direct Sockets | ❌ Unsupported | Browser security limitations |

## Feature Flags

```toml
[features]
default = ["core", "push", "noise"]
core = ["serde", "serde_json"]                          # Basic SDK functionality
noise = ["serde", "serde_json", "hex", "getrandom"]     # Noise handshake demo
push = ["serde", "serde_json", "base64", "getrandom", "js-sys", "web-sys", "wasm-bindgen-futures"]
multipath = ["serde", "serde_json", "serde-wasm-bindgen", "once_cell", "thiserror"]
hpke = ["nyx-crypto", "hex", "getrandom", "serde", "serde_json"]  # Advanced crypto
plugin = ["serde", "serde_json", "serde-wasm-bindgen", "ed25519-dalek", "sha2", "semver", "ciborium"]
```

## Usage

### Basic Initialization
```javascript
import init, { init_with_config, version, check_capabilities } from './pkg/nyx_sdk_wasm.js';

await init();
console.log('SDK Version:', version());
console.log('Capabilities:', check_capabilities());

// Initialize with configuration
const config = JSON.stringify({
    multipath: { enabled: true },
    gateway: { url: 'https://gateway.nyx.example.com' }
});
await init_with_config(config);
```

### Noise Handshake Demo
```javascript
import { noise_handshake_demo } from './pkg/nyx_sdk_wasm.js';

const initiatorConfig = JSON.stringify({
    pattern: "Noise_XX_25519_ChaChaPoly_BLAKE2s",
    psk: null,
    static_keypair: null,
    payload: "initiator_data"
});

const responderConfig = JSON.stringify({
    pattern: "Noise_XX_25519_ChaChaPoly_BLAKE2s", 
    psk: null,
    static_keypair: null,
    payload: "responder_data"
});

const result = await noise_handshake_demo(initiatorConfig, responderConfig);
const handshake = JSON.parse(result);
console.log('Handshake completed:', handshake.success);
console.log('Total time:', handshake.metrics.total_time_ms, 'ms');
```

### Push Registration
```javascript
import { nyx_register_push, check_push_support } from './pkg/nyx_sdk_wasm.js';

// Check if push is supported
const supportInfo = JSON.parse(check_push_support());
if (supportInfo.supported) {
    const clientConfig = JSON.stringify({
        application_server_key: "your_vapid_public_key",
        user_agent: navigator.userAgent
    });
    
    const result = await nyx_register_push('https://gateway.nyx.example.com', clientConfig);
    const registration = JSON.parse(result);
    console.log('Push endpoint:', registration.endpoint);
    console.log('Client ID:', registration.gateway_config.client_id);
}
```

### Multipath Management
```javascript
import { MultipathManager } from './pkg/nyx_sdk_wasm.js';

const manager = new MultipathManager();
await manager.add_path("wifi", 0.9);
await manager.add_path("cellular", 0.7);

const bestPath = manager.select_best_path();
console.log('Best path:', bestPath);

const stats = JSON.parse(manager.get_path_stats());
console.log('Path statistics:', stats);
```

### HPKE Operations (if feature enabled)
```javascript
import { hpke_available, hpke_generate_keypair, hpke_encrypt, hpke_decrypt } from './pkg/nyx_sdk_wasm.js';

if (hpke_available()) {
    const keypair = JSON.parse(await hpke_generate_keypair());
    
    const encrypted = await hpke_encrypt(keypair.public_key, "Hello, HPKE!", null);
    const encryptionResult = JSON.parse(encrypted);
    
    const decrypted = await hpke_decrypt(
        keypair.private_key, 
        encryptionResult.encapsulated_key,
        encryptionResult.ciphertext,
        null
    );
    const decryptionResult = JSON.parse(decrypted);
    console.log('Decryption successful:', decryptionResult.success);
}
```

## Build Instructions

### Standard Build
```bash
cargo build --target wasm32-unknown-unknown --features "core,noise,push"
wasm-pack build --target web --features "core,noise,push"
```

### Full Featured Build
```bash
cargo build --target wasm32-unknown-unknown --features "core,noise,push,hpke,multipath"
wasm-pack build --target web --features "core,noise,push,hpke,multipath"
```

### Testing
```bash
# Run unit tests
cargo test --features "core,noise,push,hpke"

# Run WASM-specific tests  
wasm-pack test --headless --firefox
```

## Implementation Status

- ✅ Noise handshake demonstration with XX pattern support
- ✅ Push registration helper with Service Worker integration  
- ✅ Multipath management with adaptive path selection
- ✅ HPKE encryption/decryption when feature enabled
- ✅ Cross-platform WASM compatibility (browser/Node.js/WASI)
- ✅ Comprehensive error handling and type safety
- ✅ 31 unit tests covering all features
- ✅ Production-ready build system with feature gates

## Security Considerations

- All cryptographic operations use wasm-safe APIs
- No non-deterministic host dependencies
- Service Worker integration follows browser security model
- CORS-compatible design for cross-origin deployments
- Base64 encoding for binary data transport

## Browser Support

- Chrome/Chromium 80+
- Firefox 79+  
- Safari 14+
- Edge 80+

WebAssembly and Service Worker support required.

## License

MIT OR Apache-2.0
