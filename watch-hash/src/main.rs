use blake3::Hasher;
use globset::{Glob, GlobSet, GlobSetBuilder};
use notify::{event::EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

#[derive(Debug, Serialize, Deserialize)]
struct Db {
    version: u32,
    root: String,
    created_at: u64,
    updated_at: u64,
    hashes: BTreeMap<String, String>,
}

#[derive(Debug)]
struct Args {
    root: PathBuf,
    db_path: PathBuf,
    ignore_patterns: Vec<String>,
    baseline_only: bool,
}

fn main() -> io::Result<()> {
    let args = parse_args()?;

    let root = args.root.canonicalize()?;
    let mut ignore_patterns = args.ignore_patterns;

    if let Ok(rel) = args.db_path.strip_prefix(&root) {
        ignore_patterns.push(path_to_key(rel));
    }

    let ignore_set = build_globset(&ignore_patterns)?;

    let mut db = if args.db_path.exists() {
        load_db(&args.db_path)?
    } else {
        Db {
            version: 1,
            root: root.to_string_lossy().to_string(),
            created_at: now_epoch_secs(),
            updated_at: now_epoch_secs(),
            hashes: BTreeMap::new(),
        }
    };

    if args.baseline_only || db.hashes.is_empty() {
        println!("Building baseline for {}", root.display());
        db.hashes = scan_tree(&root, &ignore_set)?;
        db.updated_at = now_epoch_secs();
        save_db(&args.db_path, &db)?;
        if args.baseline_only {
            println!("Baseline written to {}", args.db_path.display());
            return Ok(());
        }
    }

    println!("Watching {}", root.display());
    println!("DB: {}", args.db_path.display());
    if !ignore_patterns.is_empty() {
        println!("Ignore: {}", ignore_patterns.join(", "));
    }

    let (tx, rx) = channel();
    let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_millis(250))?;
    watcher.watch(&root, RecursiveMode::Recursive)?;

    loop {
        match rx.recv() {
            Ok(Ok(event)) => {
                if handle_event(&root, &ignore_set, &event, &mut db)? {
                    db.updated_at = now_epoch_secs();
                    save_db(&args.db_path, &db)?;
                }
            }
            Ok(Err(err)) => eprintln!("watch error: {err}"),
            Err(err) => {
                eprintln!("channel error: {err}");
                break;
            }
        }
    }

    Ok(())
}

fn parse_args() -> io::Result<Args> {
    let mut root = None;
    let mut db_path: Option<PathBuf> = None;
    let mut ignore_patterns = Vec::new();
    let mut baseline_only = false;

    let mut iter = env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--db" => {
                let value = iter.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--db requires a value")
                })?;
                db_path = Some(PathBuf::from(value));
            }
            "--ignore" => {
                let value = iter.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--ignore requires a value")
                })?;
                ignore_patterns.push(value);
            }
            "--baseline" => baseline_only = true,
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            other => {
                if root.is_none() {
                    root = Some(PathBuf::from(other));
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Unexpected argument: {other}"),
                    ));
                }
            }
        }
    }

    let root = root.unwrap_or_else(|| PathBuf::from("."));
    let db_path = db_path.unwrap_or_else(|| root.join(".watch-hash.json"));

    Ok(Args {
        root,
        db_path,
        ignore_patterns,
        baseline_only,
    })
}

fn print_help() {
    println!("watch-hash <path> [--db <file>] [--ignore <glob>]... [--baseline]");
    println!();
    println!("Examples:");
    println!("  watch-hash ./project");
    println!("  watch-hash ./project --ignore '**/target/**' --ignore '**/*.tmp'");
    println!("  watch-hash ./project --baseline");
}

fn build_globset(patterns: &[String]) -> io::Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).map_err(|err| {
            io::Error::new(io::ErrorKind::InvalidInput, format!("bad glob {pattern}: {err}"))
        })?;
        builder.add(glob);
    }
    builder
        .build()
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}

fn path_to_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn scan_tree(root: &Path, ignore: &GlobSet) -> io::Result<BTreeMap<String, String>> {
    let mut hashes = BTreeMap::new();
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                eprintln!("walk error: {err}");
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = match entry.path().strip_prefix(root) {
            Ok(rel) => rel,
            Err(_) => continue,
        };
        if is_ignored(rel, ignore) {
            continue;
        }
        match hash_file(entry.path()) {
            Ok(hash) => {
                hashes.insert(path_to_key(rel), hash);
            }
            Err(err) => eprintln!("hash error {}: {err}", entry.path().display()),
        }
    }
    Ok(hashes)
}

fn handle_event(
    root: &Path,
    ignore: &GlobSet,
    event: &notify::Event,
    db: &mut Db,
) -> io::Result<bool> {
    let mut changed = false;
    let kind = &event.kind;

    for path in &event.paths {
        if path == root {
            continue;
        }

        let rel = match path.strip_prefix(root) {
            Ok(rel) => rel,
            Err(_) => continue,
        };
        if is_ignored(rel, ignore) {
            continue;
        }

        if path.is_dir() {
            changed |= update_dir(root, path, ignore, db)?;
            continue;
        }

        match kind {
            EventKind::Remove(_) => {
                let key = path_to_key(rel);
                if db.hashes.remove(&key).is_some() {
                    println!("REMOVED {key}");
                    changed = true;
                }
            }
            _ => {
                if path.exists() && path.is_file() {
                    let key = path_to_key(rel);
                    if let Ok(hash) = hash_file(path) {
                        match db.hashes.get(&key) {
                            Some(old) if *old == hash => {}
                            Some(old) => {
                                println!("CHANGED {key}\n  {old} -> {hash}");
                                db.hashes.insert(key, hash);
                                changed = true;
                            }
                            None => {
                                println!("ADDED {key}\n  {hash}");
                                db.hashes.insert(key, hash);
                                changed = true;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(changed)
}

fn update_dir(root: &Path, dir: &Path, ignore: &GlobSet, db: &mut Db) -> io::Result<bool> {
    let mut changed = false;
    for entry in WalkDir::new(dir).follow_links(false) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                eprintln!("walk error: {err}");
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = match entry.path().strip_prefix(root) {
            Ok(rel) => rel,
            Err(_) => continue,
        };
        if is_ignored(rel, ignore) {
            continue;
        }
        match hash_file(entry.path()) {
            Ok(hash) => {
                let key = path_to_key(rel);
                match db.hashes.get(&key) {
                    Some(old) if *old == hash => {}
                    Some(old) => {
                        println!("CHANGED {key}\n  {old} -> {hash}");
                        db.hashes.insert(key, hash);
                        changed = true;
                    }
                    None => {
                        println!("ADDED {key}\n  {hash}");
                        db.hashes.insert(key, hash);
                        changed = true;
                    }
                }
            }
            Err(err) => eprintln!("hash error {}: {err}", entry.path().display()),
        }
    }
    Ok(changed)
}

fn is_ignored(rel: &Path, ignore: &GlobSet) -> bool {
    if ignore.is_empty() {
        return false;
    }
    ignore.is_match(rel)
}

fn hash_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Hasher::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn load_db(path: &Path) -> io::Result<Db> {
    let data = fs::read_to_string(path)?;
    serde_json::from_str(&data)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

fn save_db(path: &Path, db: &Db) -> io::Result<()> {
    let mut file = File::create(path)?;
    let data = serde_json::to_string_pretty(db)
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
    file.write_all(data.as_bytes())
}

