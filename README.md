# boobies

A serious-ish Linux package manager with an aggressively unserious CLI.

## Commands

```bash
sudo boobies bigger firefox
sudo boobies smaller firefox

sudo boobies grow
sudo boobies expand

boobies --search-in-the-valle firefox
boobies --what-is-ts firefox

boobies list
boobies version
```

## Safety

The MVP uses a configurable root directory. For development, keep the root
somewhere under your home directory:

```bash
boobies --root ./test-root bigger ./hello-1.0.0-x86_64.boob
```

Do not point an experimental package manager at `/` until you fully understand
and audit the installer.

## Package format

A `.boob` file is a gzip-compressed tar archive:

```text
metadata.json
root/
  usr/
    bin/
      hello
```

The `root/` directory is the filesystem tree that gets installed.

## Repository format

A repository is a static directory or HTTP(S) location:

```text
repo/
  database.json
  packages/
    hello-1.0.0-x86_64.boob
```

The client downloads `database.json` with `grow` and then downloads package files
on demand.

## Example repository configuration

`~/.config/boobies/config.json`:

```json
{
  "repository": "https://example.invalid/boobies/",
  "root": "/tmp/boobies-root"
}
```

For a real distribution, add signed metadata and packages before trusting
third-party repositories.
