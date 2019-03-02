// iterate over both dirs, find common (and unique?) files, compare hashes, note any differing
// hashes, combine dirs?

use failure::Error;
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use std::{
  cell::RefCell,
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
  if args.len() < 2 {
    eprintln!("usage: pack_combine <output_dir> <pack_dir...>");
    return Ok(1);
  }

  let output = Path::new(&args[0]);
  let packs: Vec<&Path> = args.iter().skip(1).map(Path::new).collect();

  if output.exists() {
    eprintln!("{} should not exist", output.to_string_lossy());
    return Ok(1);
  }

  for pack in &packs {
    if !pack.exists() || !pack.is_dir() {
      eprintln!("{} does not exist or is not a directory", pack.to_string_lossy());
      return Ok(1);
    }
  }

  println!("building file list");
  let all_paths = all_files(&packs)?;

  struct Conflict<'a> {
    path: &'a PathBuf,
    hashes: Vec<(&'a Path, Vec<u8>)>,
  }

  enum EntryStatus<'a> {
    Normal((&'a Path, &'a Path)),
    Conflict(Conflict<'a>),
  }

  println!("finding conflicts");
  let statuses: Vec<EntryStatus> = all_paths
    .par_iter()
    .map(|(path, owning_packs)| {
      if owning_packs.len() == 1 {
        return Ok(EntryStatus::Normal((owning_packs[0], path)));
      }

      let hashes: Vec<(&Path, Vec<u8>)> = owning_packs
        .par_iter()
        .map(|&p| hash_file(&p.join(&path)).map(|h| (p, h)))
        .collect::<Result<_>>()?;

      let has_conflicts = {
        let mut all_hashes: Vec<_> = hashes.iter().map(|(_, hash)| hash).collect();
        all_hashes.sort_unstable();
        all_hashes.dedup();
        all_hashes.len() != 1
      };

      if has_conflicts {
        return Ok(EntryStatus::Conflict(Conflict {
          path,
          hashes,
        }));
      }

      if !owning_packs.is_empty() {
        return Ok(EntryStatus::Normal((&owning_packs[0], path)));
      }

      unreachable!("path owned by no packs")
    })
    .collect::<Result<_>>()?;

  let mut final_paths = Vec::with_capacity(all_paths.len());
  let mut conflicts = Vec::new();

  for status in statuses {
    match status {
      EntryStatus::Normal(n) => final_paths.push(n),
      EntryStatus::Conflict(conflict) => conflicts.push(conflict),
    }
  }

  println!("resolving conflicts");
  for conflict in conflicts {
    println!("conflict: {}", conflict.path.to_string_lossy());
    for (i, (pack, hash)) in conflict.hashes.iter().enumerate() {
      println!("  enter {} to take from {}", i + 1, pack.to_string_lossy());
      println!("    sha256: {}", hex::encode(&hash));
    }

    let use_pack: u8;

    loop {
      print!("  enter choice: ");
      std::io::stdout().flush()?;
      let mut input = String::with_capacity(2);
      std::io::stdin().read_line(&mut input)?;
      if let Ok(x) = input.trim().parse::<u8>() {
        if x != 0 && x as usize <= conflict.hashes.len() {
          use_pack = x - 1;
          break;
        }
      }
    }

    final_paths.push((&conflict.hashes[use_pack as usize].0, conflict.path));
  }

  println!("creating output");
  std::fs::create_dir_all(&output)?;

  for (prefix, rel_path) in final_paths {
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

thread_local! {
  pub static HASHER: RefCell<Sha256> = RefCell::new(Sha256::default());
}

fn all_files<'a>(paths: &'a [&'a Path]) -> Result<HashMap<PathBuf, Vec<&'a Path>>> {
  let mut files: HashMap<PathBuf, Vec<&Path>> = HashMap::new();

  for path in paths {
    for entry in WalkDir::new(path) {
      let entry = entry?;

      if !entry.path().is_file() {
        continue;
      }

      files.entry(entry.path().strip_prefix(path)?.to_owned()).or_default().push(path);
    }
  }

  Ok(files)
}

fn hash_file(path: &Path) -> Result<Vec<u8>> {
  let mut hasher = HASHER.with(|h| h.borrow_mut().clone());
  let mut buf = [0; 4096];

  let mut f = File::open(path)?;

  loop {
    let read = f.read(&mut buf)?;
    if read == 0 {
      break;
    }
    hasher.input(&buf[..read]);
  }

  let hash = hasher.result_reset();

  Ok(hash.to_vec())
}
