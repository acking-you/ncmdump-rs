# Changelog

## v0.1.0

Initial release.

- Core library (`ncmdump`): NCM file parsing, AES/RC4 decryption, metadata extraction, cover art embedding
- CLI (`ncmdump-cli`): batch conversion with directory traversal, output directory, and source removal options
- C FFI (`ncmdump-ffi`): `CreateNeteaseCrypt`, `Dump`, `FixMetadata`, `DestroyNeteaseCrypt` — shared and static library builds
- CI/CD: GitHub Actions for syntax check and cross-platform release (Linux musl, macOS, Windows)
