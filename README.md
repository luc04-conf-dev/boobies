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


## Creating a `.boob` package

## Creating a `.boob` package

Boobies packages are gzip-compressed tar archives with this layout:

```text
metadata.json
root/
└── <files to be installed>

Create a package with:

python3 tools/make_package.py \
  --name hello \
  --version 1.0.0 \
  --architecture x86_64 \
  --description "Example Boobies package" \
  --input-root ./package-root \
  --output ./hello-1.0.0-x86_64.boob

The package-root directory represents the filesystem root of the package.

For example:

package-root/
└── usr/
    └── bin/
        └── hello

becomes:

root/usr/bin/hello

inside the .boob archive.

Inspect a package with:

tar -tzf hello-1.0.0-x86_64.boob

Calculate its checksum with:

sha256sum hello-1.0.0-x86_64.boob
Publishing a package

Small packages can be attached to a GitHub Release:

gh release create hello-1.0.0 \
  ./hello-1.0.0-x86_64.boob \
  --title "hello 1.0.0" \
  --notes "First release"

Large packages should not be committed directly to Git. GitHub repositories have a file-size limit, so large .boob files should be distributed as Release assets.

Add the package to the repository index:

{
  "name": "hello",
  "version": "1.0.0",
  "architecture": "x86_64",
  "description": "Example Boobies package",
  "filename": "hello-1.0.0-x86_64.boob",
  "download_url": "https://github.com/OWNER/REPOSITORY/releases/download/hello-1.0.0/hello-1.0.0-x86_64.boob",
  "sha256": "SHA256_HERE",
  "dependencies": []
}

## Forking the Boobies repository

Boobies is designed to be forked.

1. Fork this repository on GitHub.
2. Enable GitHub Pages for the fork.
3. Publish your package index under the repository's Pages URL.
4. Set the repository URL when installing:

```bash
BOOBIES_REPOSITORY="https://YOUR-USER.github.io/YOUR-REPO/examples/repo" \
bash install.sh