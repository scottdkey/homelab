# Multi-Platform FFI System

This document describes the multi-platform FFI system that generates bindings for Swift (iOS/macOS), Kotlin (Android), and TypeScript (Web/WASM).

## Architecture

```
halvor (Rust core)
    ├── halvor-ffi (C FFI for Swift)
    ├── halvor-ffi-wasm (WASM for Web)
    ├── halvor-ffi-jni (JNI for Android)
    └── halvor-ffi-macro (Code generation macros)
```

## Macros

### `#[swift_export]`
Marks a function for Swift export. Generates C FFI wrappers and Swift bindings.

### `#[kotlin_export]`
Marks a function for Kotlin/Android export. Generates JNI wrappers and Kotlin bindings.

### `#[wasm_export]`
Marks a function for WASM/Web export. Generates wasm-bindgen wrappers and TypeScript bindings.

### `#[multi_platform_export]`
Marks a function for all platforms. Equivalent to using all three macros.

## Usage Example

```rust
use halvor_ffi_macro::multi_platform_export;

#[multi_platform_export]
pub fn discover_agents(client: &HalvorClient) -> Result<Vec<DiscoveredHost>, String> {
    client.discover_agents()
}
```

This single annotation makes the function available in:
- **Swift**: `try client.discoverAgents()`
- **Kotlin**: `client.discoverAgents()`
- **TypeScript**: `await wasmModule.discoverAgents()`

## Build Process

1. **Rust Compilation**: Functions are compiled with platform-specific targets
2. **Code Generation**: Build scripts generate bindings for each platform
3. **Integration**: Generated code is automatically included in each platform's build

## Platform-Specific Builds

### Swift (iOS/macOS)
```bash
make build-ios    # iOS
make build-mac    # macOS
```

### Android
```bash
make android-jni-build  # Build JNI libraries
make android-build      # Build Android library
```

### Web
```bash
make web-wasm-build  # Build WASM module
make web-build       # Build Svelte app
```

### All Platforms
```bash
make build-all-platforms
```

## Development Workflows

### Swift Development
```bash
make dev-ios   # iOS with simulator
make dev-mac   # macOS app
```

### Web Development
```bash
make web-dev   # Docker dev mode with hot reload
```

### Android Development
Build JNI once, then use Android Studio for app development.

## Generated Files

- **Swift**: `halvor-swift/Sources/HalvorSwiftFFI/halvor_ffi/generated_swift_bindings.swift`
- **Kotlin**: `halvor-android/src/main/kotlin/dev/scottkey/halvor/GeneratedBindings.kt`
- **TypeScript**: `halvor-web/src/lib/halvor-ffi/generated-bindings.ts`

All generated files are automatically included in their respective build systems.

