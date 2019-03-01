# pack_combine

Combine an arbitrary amount of directories, prompting on files that are actually different.

Made for combining Breath of the Wild packs.

## Usage

```plaintext
pack_combine <output_dir> <pack_dir...>
```

Specify the output directory, which shouldn't exist, as the first argument, then specify input
directories after.

Note that all files from all directories will be hashed first (SHA256), then file hashes will be
compared, prompting which file to use if any hash is different.
