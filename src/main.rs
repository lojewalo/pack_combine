// iterate over both dirs, find common (and unique?) files, compare hashes, note any differing
// hashes, combine dirs?

use failure::Error;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use std::{
  collections::{HashMap, HashSet},
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

  println!("hashing packs");
  let pack_hashes = packs.iter().map(|x| pack_hashes(x)).collect::<Result<Vec<_>>>()?;

  let all_paths: HashSet<&PathBuf> = pack_hashes.iter().flat_map(|x| x.keys().collect::<HashSet<_>>()).collect();

  let mut final_paths = Vec::with_capacity(pack_hashes[0].len());

  println!("building file list");
  for path in all_paths {
    let hashes: Vec<(usize, &[u8])> = pack_hashes
      .iter()
      .enumerate()
      .flat_map(|(i, x)| x.get(path).map(|h| (i, h.as_slice())))
      .collect();

    let has_conflicts = {
      let mut all_hashes: Vec<_> = hashes.iter().map(|(_, hash)| hash).collect();
      all_hashes.sort();
      all_hashes.dedup();
      all_hashes.len() != 1
    };

    if has_conflicts {
      let mut coll_out = false;
      for (i, hash) in hashes {
        if !coll_out {
          println!("collision: {}", path.to_string_lossy());
          coll_out = true;
        }
        println!("  enter {} to take from {}", i + 1, packs[i].to_string_lossy());
        println!("    sha256: {}", hex::encode(hash));
      }

      let use_pack: u8;

      loop {
        print!("  enter choice: ");
        std::io::stdout().flush()?;
        let mut input = String::with_capacity(2);
        std::io::stdin().read_line(&mut input)?;
        if let Ok(x) = input.trim().parse::<u8>() {
          if x != 0 && x as usize <= packs.len() {
            use_pack = x - 1;
            break;
          }
        }
      }

      final_paths.push((&packs[use_pack as usize], path));

      continue;
    }

    if !hashes.is_empty() {
      final_paths.push((&packs[hashes[0].0], path));
    }
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

fn pack_hashes(pack: &Path) -> Result<HashMap<PathBuf, Vec<u8>>> {
  let mut hasher = Sha256::default();

  let mut hashes = HashMap::new();
  let mut buf = [0; 4096];

  for entry in WalkDir::new(pack) {
    let entry = entry?;

    if entry.path().is_dir() {
      continue;
    }

    let mut f = File::open(entry.path())?;

    loop {
      let read = f.read(&mut buf)?;
      if read == 0 {
        break;
      }
      hasher.input(&buf[..read]);
    }

    let hash = hasher.result_reset();
    let rel_path = entry.path().strip_prefix(pack)?;
    hashes.insert(rel_path.to_path_buf(), hash.to_vec());
  }

  Ok(hashes)
}
