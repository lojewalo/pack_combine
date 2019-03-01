// iterate over both dirs, find common (and unique?) files, compare hashes, note any differing
// hashes, combine dirs?

use failure::Error;
use openssl::hash::{Hasher, MessageDigest};
use walkdir::WalkDir;

use std::{
  collections::HashMap,
  fs::File,
  io::{Read, Write},
  path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, Error>;

fn main() {
  match inner() {
    Ok(e) => std::process::exit(e),
    Err(e) => {
      eprintln!("{:?}", e);
      std::process::exit(1);
    },
  }
}

fn inner() -> Result<i32> {
  let args: Vec<String> = std::env::args().skip(1).collect();
  if args.len() != 3 {
    eprintln!("usage: pack_compare <pack1_dir> <pack2_dir> <output_dir>");
    return Ok(0);
  }

  let pack1 = Path::new(&args[0]);
  let pack2 = Path::new(&args[1]);
  let output = Path::new(&args[2]);

  if !pack1.exists() || !pack1.is_dir() {
    eprintln!("{} does not exist or is not a directory", pack1.to_string_lossy());
    return Ok(1);
  }
  if !pack2.exists() || !pack2.is_dir() {
    eprintln!("{} does not exist or is not a directory", pack2.to_string_lossy());
    return Ok(1);
  }
  if output.exists() {
    eprintln!("{} should not exist", output.to_string_lossy());
    return Ok(1);
  }

  println!("hashing packs");
  let pack1_hashes = pack_hashes(&pack1)?;
  let pack2_hashes = pack_hashes(&pack2)?;

  println!("pack 1 len: {}", pack1_hashes.len());
  println!("pack 2 len: {}", pack2_hashes.len());

  let mut all_paths = Vec::with_capacity(pack1_hashes.len() + pack2_hashes.len());

  println!("building file list");
  for (pack1_path, pack1_hash) in pack1_hashes {
    let pack2_hash = match pack2_hashes.get(&pack1_path) {
      Some(x) => x,
      None => {
        all_paths.push((&pack1, pack1_path));
        continue;
      },
    };

    if pack1_hash != *pack2_hash {
      println!("collision: {}", pack1_path.to_string_lossy());
      println!("  enter 1 to take from {}", pack1.to_string_lossy());
      println!("  enter 2 to take from {}", pack2.to_string_lossy());
      let mut input = String::with_capacity(1);
      let use_pack1: bool;

      loop {
        print!("  enter choice [1/2]: ");
        std::io::stdout().flush()?;
        std::io::stdin().read_line(&mut input)?;
        let trimmed = input.trim();
        if trimmed == "1" {
          use_pack1 = true;
          break;
        }
        if trimmed == "2" {
          use_pack1 = false;
          break;
        }
      }

      if use_pack1 {
        all_paths.push((&pack1, pack1_path));
      } else {
        all_paths.push((&pack2, pack1_path));
      }

      continue;
    }

    // files are the same, so which one doesn't matter
    all_paths.push((&pack1, pack1_path));
  }

  println!("creating output");
  std::fs::create_dir_all(&output)?;

  for (prefix, rel_path) in all_paths {
    println!("{}", rel_path.to_string_lossy());
    let out_path = output.join(&rel_path);
    let in_path = prefix.join(&rel_path);

    if let Some(p) = out_path.parent() {
      std::fs::create_dir_all(p)?;
    }
    std::fs::copy(in_path, out_path)?;
  }

  Ok(0)
}

fn pack_hashes(pack: &Path) -> Result<HashMap<PathBuf, Vec<u8>>> {
  let sha256 = MessageDigest::sha256();

  let mut hashes = HashMap::new();
  let mut buf = [0; 4096];

  for entry in WalkDir::new(pack) {
    let entry = entry?;

    if entry.path().is_dir() {
      continue;
    }

    let mut hasher = Hasher::new(sha256)?;

    let mut f = File::open(entry.path())?;

    loop {
      let read = f.read(&mut buf)?;
      if read == 0 {
        break;
      }
      hasher.update(&buf[..read])?;
    }

    let hash = hasher.finish()?;
    let rel_path = entry.path().strip_prefix(pack)?;
    hashes.insert(rel_path.to_path_buf(), hash.to_vec());
  }

  Ok(hashes)
}
