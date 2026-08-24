#!/usr/bin/env python3
"""
Create a minimal .boob package.

Example:
  python3 tools/make_package.py \
      --name hello \
      --version 1.0.0 \
      --input-root package-root \
      --output hello-1.0.0-x86_64.boob
"""

import argparse
import json
import os
import tarfile
import tempfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--name", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--architecture", default="x86_64")
    parser.add_argument("--description", default="")
    parser.add_argument("--input-root", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    metadata = {
        "name": args.name,
        "version": args.version,
        "architecture": args.architecture,
        "description": args.description,
        "dependencies": [],
    }

    with tempfile.TemporaryDirectory() as tmp:
        tmp = os.path.abspath(tmp)
        with open(os.path.join(tmp, "metadata.json"), "w", encoding="utf-8") as f:
            json.dump(metadata, f, indent=2)

        root_target = os.path.join(tmp, "root")
        os.symlink(os.path.abspath(args.input_root), root_target)

        with tarfile.open(args.output, "w:gz", dereference=True) as tar:
            tar.add(os.path.join(tmp, "metadata.json"), arcname="metadata.json")
            tar.add(os.path.abspath(args.input_root), arcname="root")

    print(f"created {args.output}")


if __name__ == "__main__":
    main()
