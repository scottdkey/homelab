fastlane documentation
----

# Installation

Make sure you have the latest version of the Xcode command line tools installed:

```sh
xcode-select --install
```

For _fastlane_ installation instructions, see [Installing _fastlane_](https://docs.fastlane.tools/#installing-fastlane)

# Available Actions

## iOS

### ios sync_certificates

```sh
[bundle exec] fastlane ios sync_certificates
```

Download certificates and provisioning profiles for iOS

### ios sync_certificates_readonly

```sh
[bundle exec] fastlane ios sync_certificates_readonly
```

Download certificates in readonly mode (for CI/CD)

### ios build_ios

```sh
[bundle exec] fastlane ios build_ios
```

Build and sign iOS XCFramework

### ios ios_build_app

```sh
[bundle exec] fastlane ios ios_build_app
```

Build iOS app (Rust FFI + Xcode app)

### ios sign_ios

```sh
[bundle exec] fastlane ios sign_ios
```

Sign iOS XCFramework

### ios sign_app

```sh
[bundle exec] fastlane ios sign_app
```

Sign iOS app bundle

### ios ios_upload_to_app_store

```sh
[bundle exec] fastlane ios ios_upload_to_app_store
```

Upload iOS app to App Store Connect

----


## Mac

### mac sync_certificates

```sh
[bundle exec] fastlane mac sync_certificates
```

Download certificates and provisioning profiles for macOS

### mac sync_certificates_readonly

```sh
[bundle exec] fastlane mac sync_certificates_readonly
```

Download certificates in readonly mode (for CI/CD)

### mac build_mac

```sh
[bundle exec] fastlane mac build_mac
```

Build and sign macOS XCFramework

### mac mac_build_app

```sh
[bundle exec] fastlane mac mac_build_app
```

Build macOS app (Rust FFI + Xcode app)

### mac sign_mac

```sh
[bundle exec] fastlane mac sign_mac
```

Sign macOS XCFramework

### mac sign_app

```sh
[bundle exec] fastlane mac sign_app
```

Sign macOS app bundle

----

This README.md is auto-generated and will be re-generated every time [_fastlane_](https://fastlane.tools) is run.

More information about _fastlane_ can be found on [fastlane.tools](https://fastlane.tools).

The documentation of _fastlane_ can be found on [docs.fastlane.tools](https://docs.fastlane.tools).
