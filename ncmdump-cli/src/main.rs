use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(
    name = "ncmdump",
    version,
    about = "NCM decryptor & Netease Cloud Music CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Decrypt NCM files to MP3/FLAC
    Dump {
        /// NCM files to convert
        files: Vec<PathBuf>,
        /// Process all NCM files in directory
        #[arg(short, long, value_name = "PATH")]
        directory: Option<PathBuf>,
        /// Recursive directory traversal (with -d)
        #[arg(short, long)]
        recursive: bool,
        /// Output directory
        #[arg(short, long, value_name = "PATH")]
        output: Option<PathBuf>,
        /// Remove source file after successful conversion
        #[arg(short = 'm', long = "remove")]
        remove: bool,
    },
    /// Set login cookie (`MUSIC_U`)
    Login {
        /// `MUSIC_U` cookie value
        #[arg(required_unless_present = "check")]
        music_u: Option<String>,
        /// Check current login status
        #[arg(long)]
        check: bool,
    },
    /// Clear saved session
    Logout,
    /// Search for tracks, albums, artists, or playlists
    Search {
        /// Search keyword
        keyword: String,
        /// Search type
        #[arg(short = 't', long, default_value = "track")]
        r#type: SearchKind,
        /// Max results
        #[arg(short, long, default_value = "20")]
        limit: u64,
    },
    /// Show track details
    Info {
        /// Track ID
        track_id: u64,
    },
    /// Get track lyrics
    Lyric {
        /// Track ID
        track_id: u64,
    },
    /// Download a track
    Download {
        /// Track ID
        track_id: u64,
        /// Audio quality
        #[arg(short, long, default_value = "exhigh")]
        quality: QualityArg,
        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Show playlist details
    Playlist {
        /// Playlist ID
        playlist_id: u64,
    },
    /// Show current user info
    Me,
}

#[derive(Clone, ValueEnum)]
enum SearchKind {
    Track,
    Album,
    Artist,
    Playlist,
}

#[derive(Clone, ValueEnum)]
enum QualityArg {
    Standard,
    Higher,
    Exhigh,
    Lossless,
}

impl From<SearchKind> for netease_api::types::SearchType {
    fn from(k: SearchKind) -> Self {
        match k {
            SearchKind::Track => Self::Track,
            SearchKind::Album => Self::Album,
            SearchKind::Artist => Self::Artist,
            SearchKind::Playlist => Self::Playlist,
        }
    }
}

impl From<QualityArg> for netease_api::types::Quality {
    fn from(q: QualityArg) -> Self {
        match q {
            QualityArg::Standard => Self::Standard,
            QualityArg::Higher => Self::Higher,
            QualityArg::Exhigh => Self::Exhigh,
            QualityArg::Lossless => Self::Lossless,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Dump {
            files,
            directory,
            recursive,
            output,
            remove,
        } => cmd_dump(
            files,
            directory.as_ref(),
            recursive,
            output.as_ref(),
            remove,
        ),
        Command::Login { music_u, check } => cmd_login(music_u, check),
        Command::Logout => cmd_logout(),
        Command::Search {
            keyword,
            r#type,
            limit,
        } => cmd_search(&keyword, r#type, limit),
        Command::Info { track_id } => cmd_info(track_id),
        Command::Lyric { track_id } => cmd_lyric(track_id),
        Command::Download {
            track_id,
            quality,
            output,
        } => cmd_download(track_id, quality, output),
        Command::Playlist { playlist_id } => cmd_playlist(playlist_id),
        Command::Me => cmd_me(),
    }
}

// ── dump ──

fn cmd_dump(
    mut files: Vec<PathBuf>,
    directory: Option<&PathBuf>,
    recursive: bool,
    output: Option<&PathBuf>,
    remove: bool,
) -> Result<()> {
    if let Some(dir) = directory {
        if recursive {
            for entry in WalkDir::new(dir)
                .into_iter()
                .filter_map(std::result::Result::ok)
            {
                if entry.path().extension().is_some_and(|e| e == "ncm") {
                    files.push(entry.into_path());
                }
            }
        } else {
            for entry in std::fs::read_dir(dir).context("failed to read directory")? {
                let path = entry?.path();
                if path.extension().is_some_and(|e| e == "ncm") {
                    files.push(path);
                }
            }
        }
    }

    if files.is_empty() {
        eprintln!("No NCM files specified. Use --help for usage.");
        std::process::exit(1);
    }

    let output_dir = output.map(PathBuf::as_path);
    for file in &files {
        match ncmdump::convert(file, output_dir) {
            Ok(out) => {
                println!("{} -> {}", file.display(), out.display());
                if remove {
                    if let Err(e) = std::fs::remove_file(file) {
                        eprintln!("warning: failed to remove {}: {e}", file.display());
                    }
                }
            }
            Err(e) => eprintln!("error: {}: {e}", file.display()),
        }
    }
    Ok(())
}

// ── login / logout ──

fn cmd_login(music_u: Option<String>, check: bool) -> Result<()> {
    use netease_api::auth::Session;

    if check {
        let session = Session::load()?;
        if session.is_logged_in() {
            let client = netease_api::NeteaseClient::with_session(session)?;
            match client.user_info() {
                Ok(profile) => println!("Logged in as: {} (id={})", profile.nickname, profile.id),
                Err(e) => println!("Session exists but validation failed: {e}"),
            }
        } else {
            println!("Not logged in.");
        }
        return Ok(());
    }

    let music_u = music_u.context("MUSIC_U value required")?;
    let session = Session {
        music_u: Some(music_u),
    };
    session.save()?;
    println!("Session saved.");
    Ok(())
}

fn cmd_logout() -> Result<()> {
    netease_api::auth::Session::clear()?;
    println!("Session cleared.");
    Ok(())
}

// ── search ──

fn cmd_search(keyword: &str, kind: SearchKind, limit: u64) -> Result<()> {
    let client = netease_api::NeteaseClient::new()?;
    let search_type = kind.into();
    let result = client.search(keyword, search_type, limit, 0)?;

    println!("Total: {}\n", result.total);

    if let Some(tracks) = &result.tracks {
        for t in tracks {
            let artists: Vec<&str> = t.artists.iter().map(|a| a.name.as_str()).collect();
            println!(
                "  [{}] {} - {} ({})",
                t.id,
                artists.join(", "),
                t.name,
                t.album.name,
            );
        }
    }
    if let Some(albums) = &result.albums {
        for a in albums {
            println!("  [{}] {}", a.id, a.name);
        }
    }
    if let Some(artists) = &result.artists {
        for a in artists {
            println!("  [{}] {}", a.id, a.name);
        }
    }
    if let Some(playlists) = &result.playlists {
        for p in playlists {
            println!("  [{}] {} ({} tracks)", p.id, p.name, p.track_count);
        }
    }
    Ok(())
}

// ── info / lyric / download ──

fn cmd_info(track_id: u64) -> Result<()> {
    let client = netease_api::NeteaseClient::new()?;
    let t = client.track_detail(track_id)?;
    let artists: Vec<&str> = t.artists.iter().map(|a| a.name.as_str()).collect();
    println!("Track:    {} (id={})", t.name, t.id);
    println!("Artists:  {}", artists.join(", "));
    println!("Album:    {} (id={})", t.album.name, t.album.id);
    println!(
        "Duration: {}:{:02}",
        t.duration_ms / 60000,
        (t.duration_ms / 1000) % 60
    );
    Ok(())
}

fn cmd_lyric(track_id: u64) -> Result<()> {
    let client = netease_api::NeteaseClient::new()?;
    let lyric = client.track_lyric(track_id)?;
    if let Some(lrc) = &lyric.lrc {
        println!("{lrc}");
    }
    if let Some(tlyric) = &lyric.tlyric {
        println!("\n--- Translation ---\n{tlyric}");
    }
    if lyric.lrc.is_none() && lyric.tlyric.is_none() {
        println!("No lyrics available.");
    }
    Ok(())
}

fn cmd_download(track_id: u64, quality: QualityArg, output: Option<PathBuf>) -> Result<()> {
    let client = netease_api::NeteaseClient::new()?;
    let q: netease_api::types::Quality = quality.into();

    let dest = if let Some(p) = output {
        p
    } else {
        let url = client.track_url(track_id, q)?;
        let ext = if url.contains(".flac") { "flac" } else { "mp3" };
        PathBuf::from(format!("{track_id}.{ext}"))
    };

    let size = client.download_track(track_id, q, &dest)?;
    println!("Downloaded {} ({} bytes)", dest.display(), size);
    Ok(())
}

// ── playlist ──

fn cmd_playlist(playlist_id: u64) -> Result<()> {
    let client = netease_api::NeteaseClient::new()?;
    let p = client.playlist_detail(playlist_id)?;
    println!("Playlist: {} (id={})", p.name, p.id);
    println!("Tracks:   {}", p.track_count);
    if let Some(desc) = &p.description {
        println!("Desc:     {desc}");
    }
    if let Some(creator) = &p.creator {
        println!("Creator:  {} (id={})", creator.name, creator.id);
    }
    if let Some(tracks) = &p.tracks {
        println!();
        for t in tracks {
            let artists: Vec<&str> = t.artists.iter().map(|a| a.name.as_str()).collect();
            println!("  [{}] {} - {}", t.id, artists.join(", "), t.name);
        }
    }
    Ok(())
}

// ── me ──

fn cmd_me() -> Result<()> {
    let client = netease_api::NeteaseClient::new()?;
    let profile = client.user_info()?;
    println!("User:   {} (id={})", profile.nickname, profile.id);
    if let Some(url) = &profile.avatar_url {
        println!("Avatar: {url}");
    }
    Ok(())
}
