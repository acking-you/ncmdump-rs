# ncmdump-rs

Rust reimplementation of [taurusxin/ncmdump](https://github.com/taurusxin/ncmdump) — convert NetEase Cloud Music `.ncm` files to standard MP3/FLAC, plus a built-in Netease Cloud Music API client for search, download, and more.

## Why Rust?

- Memory-safe decryption with zero `unsafe` in core library
- Single static binary, no runtime dependencies
- Cross-platform CI/CD: prebuilt binaries for Linux (musl), macOS, and Windows
- C FFI with near-zero OS dependencies — drop-in `.so`/`.dylib`/`.dll`/`.a`/`.lib`

## Project Structure

| Crate | Description |
|---|---|
| `ncmdump` | Core library: NCM parsing, AES/RC4 decryption, metadata & cover art |
| `netease-api` | Netease Cloud Music API client: search, track info/URL/lyric, playlist, user |
| `ncmdump-cli` | CLI tool: NCM decryption + Netease API commands |
| `ncmdump-ffi` | C FFI bindings (shared + static library) |

## CLI Usage

### NCM Decryption

```bash
# Convert single file
ncmdump-cli dump song.ncm

# Convert directory recursively, output to ./output/
ncmdump-cli dump -d ./music -r -o ./output

# Remove source files after conversion
ncmdump-cli dump -d ./music -r -m
```

### Netease Cloud Music API

```bash
# Login with MUSIC_U cookie (from browser DevTools)
ncmdump-cli login <MUSIC_U>
ncmdump-cli login --check
ncmdump-cli logout

# Search
ncmdump-cli search "周杰伦 晴天"
ncmdump-cli search "赵雷" -t artist
ncmdump-cli search "华语经典" -t playlist -l 5

# Track info / lyrics / download
ncmdump-cli info <TRACK_ID>
ncmdump-cli lyric <TRACK_ID>
ncmdump-cli download <TRACK_ID> -q exhigh -o song.mp3

# Playlist detail
ncmdump-cli playlist <PLAYLIST_ID>

# Current user
ncmdump-cli me
```

Quality options: `standard` (128k) / `higher` (192k) / `exhigh` (320k) / `lossless` (FLAC).

> See [docs/netease-api.md](docs/netease-api.md) for full API documentation including request/response JSON formats.

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
