#!/usr/bin/env python3
"""
Create a minimal .boob package.

Example:
  python3 tools/make_package.py \
      --name hello \
      --version 1.0.0 \
      --input-root package-root \
      --output hello-1.0.0-x86_64.boob

Package layout:

  metadata.json
  root/
    <contents of --input-root>

The generated .boob file is a gzip-compressed tar archive.
"""

import argparse
import json
import os
import tarfile
from pathlib import Path


def build_metadata(args: argparse.Namespace) -> dict:
    """Build the metadata stored inside the package."""
    return {
        "name": args.name,
        "version": args.version,
        "architecture": args.architecture,
        "description": args.description,
        "dependencies": [],
    }


def create_package(
    input_root: Path,
    output_path: Path,
    metadata: dict,
) -> None:
    """
    Create a .boob package.

    The resulting archive contains:

        metadata.json
        root/<files from input_root>
    """

    if not input_root.exists():
        raise FileNotFoundError(
            f"input root does not exist: {input_root}"
        )

    if not input_root.is_dir():
        raise NotADirectoryError(
            f"input root is not a directory: {input_root}"
        )

    output_parent = output_path.parent
    output_parent.mkdir(parents=True, exist_ok=True)

    # Create a temporary metadata file next to the package generation process.
    metadata_path = output_parent / f".{output_path.name}.metadata.json"

    try:
        with metadata_path.open("w", encoding="utf-8") as file:
            json.dump(
                metadata,
                file,
                indent=2,
                ensure_ascii=False,
            )
            file.write("\n")

        with tarfile.open(
            output_path,
            mode="w:gz",
            dereference=True,
        ) as tar:
            # Add package metadata.
            tar.add(
                metadata_path,
                arcname="metadata.json",
                recursive=False,
            )

            # Add the contents of input_root under root/.
            for entry in sorted(input_root.iterdir(), key=lambda p: p.name):
                tar.add(
                    entry,
                    arcname=os.path.join("root", entry.name),
                    recursive=True,
                )

    finally:
        try:
            metadata_path.unlink()
        except FileNotFoundError:
            pass


def parse_arguments() -> argparse.Namespace:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(
        description="Create a minimal .boob package."
    )

    parser.add_argument(
        "--name",
        required=True,
        help="Package name.",
    )

    parser.add_argument(
        "--version",
        required=True,
        help="Package version.",
    )

    parser.add_argument(
        "--architecture",
        default="x86_64",
        help="Package architecture. Default: x86_64",
    )

    parser.add_argument(
        "--description",
        default="",
        help="Package description.",
    )

    parser.add_argument(
        "--input-root",
        required=True,
        help="Directory containing the filesystem tree to package.",
    )

    parser.add_argument(
        "--output",
        required=True,
        help="Output .boob file.",
    )

    return parser.parse_args()


def main() -> None:
    args = parse_arguments()

    input_root = Path(args.input_root).expanduser().resolve()
    output_path = Path(args.output).expanduser().resolve()

    metadata = build_metadata(args)

    create_package(
        input_root=input_root,
        output_path=output_path,
        metadata=metadata,
    )

    print(f"created {output_path}")


if __name__ == "__main__":
    main()