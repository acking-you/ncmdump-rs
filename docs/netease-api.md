# Netease Cloud Music API 文档

`netease-api` 是一个独立的 Rust crate，封装了网易云音乐 WEAPI 接口，提供搜索、歌曲详情、歌词、下载、歌单、用户信息等功能。

## 目录

- [认证](#认证)
- [加密机制](#加密机制)
- [API 端点](#api-端点)
  - [搜索](#搜索)
  - [歌曲详情](#歌曲详情)
  - [播放链接](#播放链接)
  - [歌词](#歌词)
  - [歌单详情](#歌单详情)
  - [用户信息](#用户信息)
- [数据类型](#数据类型)
- [错误处理](#错误处理)
- [CLI 命令参考](#cli-命令参考)

---

## 认证

所有需要登录的 API 调用依赖 `MUSIC_U` cookie。

### 获取方式

1. 浏览器登录 [music.163.com](https://music.163.com)
2. 打开开发者工具（F12）→ Application → Cookies
3. 复制 `MUSIC_U` 字段的值

### 存储

Cookie 持久化到 `~/.config/ncmdump/session.json`：

```json
{
  "MUSIC_U": "00AABBCC..."
}
```

### 有效期

`MUSIC_U` 通常有效数月至一年，除非主动退出登录或修改密码。

### Rust 用法

```rust
use netease_api::auth::Session;
use netease_api::NeteaseClient;

// 保存 cookie
let session = Session { music_u: Some("YOUR_MUSIC_U".into()) };
session.save().unwrap();

// 创建客户端（自动从磁盘加载 session）
let client = NeteaseClient::new().unwrap();
```

---

## 加密机制

所有请求使用 WEAPI 加密方案，与网易云网页客户端一致：

```
JSON 参数
  ↓ AES-128-CBC (preset_key="0CoJUm6Qyw8W8jud", iv="0102030405060708")
  ↓ Base64
  ↓ AES-128-CBC (random_key, iv="0102030405060708")
  ↓ Base64
  = params

random_key
  ↓ 反转字节序
  ↓ 零填充至 128 字节
  ↓ RSA modpow(e=65537, n=<1024-bit public key>)
  ↓ 十六进制编码
  = encSecKey
```

请求以 `application/x-www-form-urlencoded` 格式 POST 到 `https://music.163.com/weapi{endpoint}`：

```
params=<url_encoded_base64>&encSecKey=<256_hex_chars>
```

---

## API 端点

### 搜索

**方法**: `NeteaseClient::search(keyword, search_type, limit, offset)`

**端点**: `POST /weapi/cloudsearch/get/web`

**请求参数**:

| 参数 | 类型 | 说明 |
|------|------|------|
| `s` | string | 搜索关键词 |
| `type` | number | 搜索类型：1=歌曲, 10=专辑, 100=歌手, 1000=歌单 |
| `limit` | number | 每页数量（默认 20，最大 100） |
| `offset` | number | 分页偏移（从 0 开始） |

**响应示例**（type=1，搜索歌曲）:

```json
{
  "code": 200,
  "result": {
    "songCount": 268,
    "songs": [
      {
        "id": 1436910205,
        "name": "好想爱这个世界啊 (Live)",
        "ar": [{ "id": 861777, "name": "华晨宇" }],
        "al": {
          "id": 89741282,
          "name": "歌手·当打之年 第9期",
          "picUrl": "https://p1.music.126.net/..."
        },
        "dt": 312000
      }
    ]
  }
}
```

**其他搜索类型的响应字段**:

| type | 数组字段 | 计数字段 |
|------|----------|----------|
| 1 (歌曲) | `songs` | `songCount` |
| 10 (专辑) | `albums` | `albumCount` |
| 100 (歌手) | `artists` | `artistCount` |
| 1000 (歌单) | `playlists` | `playlistCount` |

---

### 歌曲详情

**方法**: `NeteaseClient::track_detail(id)`

**端点**: `POST /weapi/song/detail`

**请求参数**:

| 参数 | 类型 | 说明 |
|------|------|------|
| `c` | string | JSON 数组，如 `[{"id":123}]` |
| `ids` | string | ID 数组，如 `[123]` |

**响应示例**:

```json
{
  "code": 200,
  "songs": [
    {
      "id": 1974443815,
      "name": "程艾影",
      "ar": [{ "id": 6731, "name": "赵雷" }],
      "al": {
        "id": 152065218,
        "name": "署前街少年",
        "picUrl": "https://p1.music.126.net/..."
      },
      "dt": 298000
    }
  ]
}
```

**字段说明**:
- `ar` / `artists` — 歌手数组（新旧 API 字段名不同，客户端兼容两者）
- `al` / `album` — 专辑对象
- `dt` / `duration` — 时长（毫秒）

---

### 播放链接

**方法**: `NeteaseClient::track_url(id, quality)`

**端点**: `POST /weapi/song/enhance/player/url`

**请求参数**:

| 参数 | 类型 | 说明 |
|------|------|------|
| `ids` | string | ID 数组，如 `[123]` |
| `br` | number | 比特率：128000 / 192000 / 320000 / 999000 |

**音质对照表**:

| Quality | br 值 | 格式 | 要求 |
|---------|-------|------|------|
| Standard | 128000 | MP3 | 免费 |
| Higher | 192000 | MP3 | 免费/VIP |
| Exhigh | 320000 | MP3 | VIP |
| Lossless | 999000 | FLAC | VIP |

**响应示例**:

```json
{
  "code": 200,
  "data": [
    {
      "id": 1974443815,
      "url": "https://m701.music.126.net/20260221/xxx.mp3",
      "br": 320000,
      "size": 12018460,
      "type": "mp3"
    }
  ]
}
```

**重要说明**:
- `url` 为 `null` 表示歌曲不可用（版权限制、需要购买专辑、或地区限制）
- URL 是临时 CDN 链接，有效期约 20 分钟
- 服务器可能降级音质（如请求 320k 但只有 128k 版权）

---

### 歌词

**方法**: `NeteaseClient::track_lyric(id)`

**端点**: `POST /weapi/song/lyric`

**请求参数**:

| 参数 | 类型 | 说明 |
|------|------|------|
| `id` | number | 歌曲 ID |
| `lv` | number | 固定 -1（获取原始歌词） |
| `tv` | number | 固定 -1（获取翻译歌词） |

**响应示例**:

```json
{
  "code": 200,
  "lrc": {
    "lyric": "[00:00.000] 作词 : xxx\n[00:01.000] 作曲 : xxx\n[00:12.340]第一句歌词\n..."
  },
  "tlyric": {
    "lyric": "[00:12.340]First line translation\n..."
  }
}
```

**说明**:
- `lrc.lyric` — 原始歌词，LRC 时间戳格式
- `tlyric.lyric` — 翻译歌词（中↔外语），可能不存在
- 纯音乐或未上传歌词的曲目，`lrc` / `tlyric` 可能缺失或为空

---

### 歌单详情

**方法**: `NeteaseClient::playlist_detail(id)`

**端点**: `POST /weapi/v6/playlist/detail`

**请求参数**:

| 参数 | 类型 | 说明 |
|------|------|------|
| `id` | number | 歌单 ID |
| `n` | number | 返回的曲目数量（100000 = 全部） |

**响应示例**:

```json
{
  "code": 200,
  "playlist": {
    "id": 123456,
    "name": "我喜欢的音乐",
    "description": "歌单描述...",
    "coverImgUrl": "https://p1.music.126.net/...",
    "trackCount": 50,
    "creator": {
      "userId": 413184081,
      "nickname": "用户名"
    },
    "tracks": [
      {
        "id": 1974443815,
        "name": "程艾影",
        "ar": [{ "id": 6731, "name": "赵雷" }],
        "al": { "id": 152065218, "name": "署前街少年" },
        "dt": 298000
      }
    ]
  }
}
```

**说明**:
- 不传 `n` 参数时，`tracks` 数组只包含 track ID，不含完整信息
- 公开歌单不需要登录即可访问

---

### 用户信息

**方法**: `NeteaseClient::user_info()`

**端点**: `POST /weapi/nuser/account/get`

**请求参数**: `{}`（空对象，认证通过 Cookie 完成）

**响应示例**:

```json
{
  "code": 200,
  "profile": {
    "userId": 413184081,
    "nickname": "为什么我说你",
    "avatarUrl": "https://p1.music.126.net/..."
  }
}
```

**错误码**:
- `301` — 未登录或 Cookie 已过期

---

## 数据类型

### Rust 类型与 API 字段映射

| Rust 类型 | API JSON 字段 | 说明 |
|-----------|---------------|------|
| `Track.id` | `id` | 歌曲 ID (u64) |
| `Track.name` | `name` | 歌曲名 |
| `Track.artists` | `ar` 或 `artists` | 歌手数组 |
| `Track.album` | `al` 或 `album` | 专辑对象 |
| `Track.duration_ms` | `dt` 或 `duration` | 时长（毫秒） |
| `Album.pic_url` | `picUrl` | 封面图 URL |
| `Playlist.cover_url` | `coverImgUrl` | 歌单封面 URL |
| `Playlist.creator` | `creator.userId` + `creator.nickname` | 创建者 |
| `UserProfile.id` | `profile.userId` | 用户 ID |
| `UserProfile.nickname` | `profile.nickname` | 昵称 |
| `Lyric.lrc` | `lrc.lyric` | 原始歌词 (LRC) |
| `Lyric.tlyric` | `tlyric.lyric` | 翻译歌词 (LRC) |

### SearchType 枚举

| 变体 | API 值 | 搜索目标 |
|------|--------|----------|
| `Track` | 1 | 歌曲 |
| `Album` | 10 | 专辑 |
| `Artist` | 100 | 歌手 |
| `Playlist` | 1000 | 歌单 |

### Quality 枚举

| 变体 | 比特率 | 格式 |
|------|--------|------|
| `Standard` | 128 kbps | MP3 |
| `Higher` | 192 kbps | MP3 |
| `Exhigh` | 320 kbps | MP3 |
| `Lossless` | 999 kbps* | FLAC |

*999000 是哨兵值，实际无损比特率因文件而异。

---

## 错误处理

| 错误类型 | 触发场景 |
|----------|----------|
| `NeteaseError::Http` | 网络连接失败、超时、TLS 错误 |
| `NeteaseError::Api { code, message }` | API 返回非 200 状态码 |
| `NeteaseError::NotLoggedIn` | 未配置 `MUSIC_U` cookie |
| `NeteaseError::Io` | 文件读写失败（session、下载） |
| `NeteaseError::Json` | API 响应 JSON 解析失败 |
| `NeteaseError::Other` | 其他错误（如找不到配置目录） |

### 常见 API 错误码

| code | 含义 |
|------|------|
| 200 | 成功 |
| 301 | 未登录 / Cookie 过期 |
| 403 | 无权限（需要 VIP 或地区限制） |
| -460 | 请求过于频繁（反爬） |

---

## CLI 命令参考

### 登录

```bash
# 设置 MUSIC_U cookie
ncmdump-cli login <MUSIC_U>

# 检查登录状态
ncmdump-cli login --check

# 退出登录
ncmdump-cli logout
```

### 搜索

```bash
# 搜索歌曲（默认）
ncmdump-cli search "关键词"

# 搜索专辑
ncmdump-cli search "关键词" -t album

# 搜索歌手
ncmdump-cli search "关键词" -t artist

# 搜索歌单
ncmdump-cli search "关键词" -t playlist

# 限制结果数量
ncmdump-cli search "关键词" -l 5
```

### 歌曲信息

```bash
ncmdump-cli info <TRACK_ID>
```

输出：歌名、歌手、专辑、时长。

### 歌词

```bash
# 输出到终端
ncmdump-cli lyric <TRACK_ID>

# 保存到文件
ncmdump-cli lyric <TRACK_ID> > song.lrc
```

### 下载

```bash
# 默认 320kbps，保存为 <TRACK_ID>.mp3
ncmdump-cli download <TRACK_ID>

# 指定音质和输出路径
ncmdump-cli download <TRACK_ID> -q lossless -o song.flac

# 音质选项：standard / higher / exhigh / lossless
```

### 歌单

```bash
ncmdump-cli playlist <PLAYLIST_ID>
```

输出：歌单名、曲目数、创建者、全部曲目列表。

### 用户信息

```bash
ncmdump-cli me
```

### NCM 解密（原有功能）

```bash
ncmdump-cli dump file.ncm
ncmdump-cli dump -d ./music -r -o ./output
ncmdump-cli dump -d ./music -r -m
```

---

## Bilibili API

> 源码: `bilibili-api/src/`

### 认证: QR 码登录

Bilibili 使用二维码扫码登录，流程如下：

1. 调用 `qr_generate()` 获取二维码 URL 和轮询密钥
   - 端点: `https://passport.bilibili.com/x/passport-login/web/qrcode/generate`
   - 返回: `url`（二维码内容）+ `qrcode_key`（轮询密钥）

2. 用户使用 Bilibili 手机客户端扫码

3. 轮询 `qr_poll(qrcode_key)` 检查状态
   - 端点: `https://passport.bilibili.com/x/passport-login/web/qrcode/poll`
   - 状态码:
     - `86101` → 等待扫码
     - `86090` → 已扫码，等待确认
     - `0` → 登录成功，提取 session
     - `86038` → 二维码过期

4. 登录成功后从 `Set-Cookie` 提取会话字段，保存到磁盘

#### Session 存储

- 路径: `~/.config/ncmdump/bilibili_session.json`
- 字段:
  - `sessdata` — 主会话 cookie
  - `bili_jct` — CSRF token
  - `dede_user_id` — 用户 ID
  - `buvid3` / `buvid4` — 设备指纹

### WBI 签名机制

Bilibili API 使用 WBI 签名防止请求篡改：

1. 从 `/x/web-interface/nav` 获取 `img_key` 和 `sub_key`（从 `wbi_img.img_url` / `wbi_img.sub_url` 提取文件名）
2. 通过固定 64 位打乱表生成 `mixin_key`（取前 32 字符）
3. 添加 `wts`（当前 UTC 时间戳）到参数
4. 按键名排序，过滤值中的特殊字符（`!`, `'`, `(`, `)`, `*`）
5. 计算 `w_rid = MD5(query_string + mixin_key)`
6. 将 `wts` 和 `w_rid` 附加到请求参数

所有需要认证的 API 端点均通过 `wbi_get()` 发送签名请求。

### API 端点

#### 搜索视频

- 端点: `/x/web-interface/wbi/search/type`（WBI 签名）
- 参数: `search_type=video`, `keyword`, `page`, `page_size`
- 返回: `SearchResult { num_results, page, page_size, results: Vec<VideoItem> }`

#### 视频详情

- 端点: `/x/web-interface/view`（WBI 签名）
- 参数: `bvid`
- 返回: `VideoDetail { bvid, aid, cid, title, pic, desc, duration, owner, pages }`

#### 音频下载（DASH + ffmpeg）

流程:
1. 获取视频详情 → 取 `cid`
2. 请求 DASH 流: `/x/player/wbi/playurl`（WBI 签名），参数 `bvid`, `cid`, `fnval=4048`
3. 选择最佳音频流（优先 FLAC > 最高带宽 AAC）
4. 下载原始 m4s 到临时文件
5. 使用 ffmpeg 转换为目标格式（MP3: `-codec:a libmp3lame -b:a 320k`; FLAC: `-codec:a flac`）

音频质量等级:
| ID | 质量 | 说明 |
|---|---|---|
| 30216 | Low | 64 kbps AAC |
| 30232 | Normal | 132 kbps AAC |
| 30280 | High | 192 kbps AAC（需登录）|
| 30250 | Dolby | 杜比全景声（大会员）|
| 30251 | HiRes | Hi-Res FLAC（大会员）|

#### 用户信息

- 端点: `/x/web-interface/nav`（WBI 签名）
- 返回: `UserInfo { is_login, mid, name, face, vip_status }`

### 数据类型映射

| 类型 | 字段 | 说明 |
|---|---|---|
| `VideoItem` | `bvid, title, description, author, mid, pic, duration, play` | 搜索结果项 |
| `VideoDetail` | `bvid, aid, cid, title, pic, desc, duration, owner, pages` | 视频详情 |
| `VideoOwner` | `mid, name` | UP 主信息 |
| `VideoPart` | `cid, page, part, duration` | 视频分 P |
| `SearchResult` | `num_results, page, page_size, results` | 搜索结果 |
| `DashAudio` | `id, base_url, backup_url, bandwidth, codecs, mime_type` | DASH 音频流 |
| `DashInfo` | `audio, flac` | DASH 流信息 |
| `UserInfo` | `is_login, mid, name, face, vip_status` | 用户信息 |
| `AudioQuality` | 枚举 | 音频质量等级 |
| `AudioFormat` | `Mp3, Flac` | 输出格式 |

### 错误处理

`BilibiliError` 枚举（`bilibili-api/src/error.rs`）:

| 变体 | 说明 |
|---|---|
| `Http(reqwest::Error)` | HTTP 传输错误 |
| `Api { code, message }` | API 返回非零 code |
| `NotLoggedIn` | 未登录（无有效会话）|
| `Io(std::io::Error)` | 文件 I/O 错误 |
| `Json(serde_json::Error)` | JSON 解析错误 |
| `Ffmpeg(String)` | ffmpeg 转换失败 |
| `QrLogin(String)` | 二维码登录流程错误 |
| `Other(String)` | 其他错误 |

### CLI 命令参考

```bash
# 二维码登录
ncmdump-cli bili-login
# 检查登录状态
ncmdump-cli bili-login --check

# 登出（清除会话）
ncmdump-cli bili-logout

# 搜索视频
ncmdump-cli bili-search "关键词" [-l 20] [-p 1]

# 视频详情
ncmdump-cli bili-info BV1xx411c7mD

# 下载音频（默认 mp3）
ncmdump-cli bili-download BV1xx411c7mD [-f mp3|flac] [-o output.mp3]

# 当前用户信息
ncmdump-cli bili-me
```

参考: `bilibili-api/src/client.rs`, `types.rs`, `error.rs`, `ncmdump-cli/src/main.rs:88-131`

