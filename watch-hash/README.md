# watch-hash

Simple file integrity monitor that watches a folder and records hashes.

## Usage

```bash
watch-hash <path> [--db <file>] [--ignore <glob>]... [--baseline]
```

Examples:

```bash
watch-hash ./project
watch-hash ./project --ignore '**/target/**' --ignore '**/*.tmp'
watch-hash ./project --baseline
```

## Notes

- Hashing uses BLAKE3.
- The database is a JSON file (`.watch-hash.json` by default).
- `--baseline` forces a fresh snapshot and exits.

