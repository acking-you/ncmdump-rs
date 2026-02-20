# ncmdump-rs

Rust reimplementation of [taurusxin/ncmdump](https://github.com/taurusxin/ncmdump) — convert NetEase Cloud Music `.ncm` files to standard MP3/FLAC.

## Why Rust?

- Memory-safe decryption with zero `unsafe` in core library
- Single static binary, no runtime dependencies
- Cross-platform CI/CD: prebuilt binaries for Linux (musl), macOS, and Windows
- C FFI with near-zero OS dependencies — drop-in `.so`/`.dylib`/`.dll`/`.a`/`.lib`

## Project Structure

| Crate | Description |
|---|---|
| `ncmdump` | Core library: NCM parsing, AES/RC4 decryption, metadata & cover art |
| `ncmdump-cli` | CLI tool for batch conversion |
| `ncmdump-ffi` | C FFI bindings (shared + static library) |

## CLI Usage

```bash
# Convert single file
ncmdump-cli song.ncm

# Convert directory recursively, output to ./output/
ncmdump-cli -d ./music -r -o ./output

# Remove source files after conversion
ncmdump-cli -d ./music -r -m
```

## FFI API

```c
NeteaseCrypt* CreateNeteaseCrypt(const char* path);
int           Dump(NeteaseCrypt* handle, const char* output_path);
void          FixMetadata(NeteaseCrypt* handle);
void          DestroyNeteaseCrypt(NeteaseCrypt* handle);
```

## Building

```bash
cargo build --release
```

## License

[MIT](LICENSE)
