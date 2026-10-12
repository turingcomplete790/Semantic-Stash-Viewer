#!/usr/bin/env python3
"""Seed the Testing Stash (:9998) with images for 012 (specs/012-browse-images, research R8).

Copies a sample of real images from Production (:9999, read-only: queries and file reads only)
(plain files, or members extracted from the zip galleries that hold most of them) into a folder of
Testing's own library under neutral names, scans only that folder, tags some of
them with 008's performers and tag, and saves an image filter. Production has no image clips and no
broken files, so one generated clip and one truncated JPEG cover those cases. Idempotent: copied
files are kept, a re-scan is harmless, assignments are additive. Prints ids and counts only (never a
key, a title, or a path).

Usage: seed.py --creds <Testing creds file>
"""

import argparse
import json
import os
import random
import re
import shutil
import subprocess
import sys
import time
import urllib.request
import zipfile

TESTING = "http://localhost:9998"
PRODUCTION = "http://localhost:9999"
PERFORMER_A, PERFORMER_B, ORDINARY_TAG = "2", "3", "6"
ORDINARY_COUNT = 1150
ANIMATED_EACH = 20
LARGEST = 15


def die(msg):
    print(f"seed: {msg}", file=sys.stderr)
    sys.exit(1)


class Stash:
    def __init__(self, url, key=None):
        self.url = url.rstrip("/") + "/graphql"
        self.key = key

    def gql(self, query, variables=None):
        headers = {"Content-Type": "application/json"}
        if self.key:
            headers["ApiKey"] = self.key
        body = json.dumps({"query": query, "variables": variables or {}}).encode()
        req = urllib.request.Request(self.url, data=body, headers=headers)
        with urllib.request.urlopen(req, timeout=300) as resp:
            out = json.load(resp)
        if out.get("errors"):
            die("GraphQL error: " + "; ".join(e.get("message", "?") for e in out["errors"]))
        return out["data"]

    def wait_job(self, job_id):
        while True:
            job = self.gql("query($id: ID!) { findJob(input: {id: $id}) { status } }", {"id": job_id})[
                "findJob"
            ]
            if job is None or job["status"] in ("FINISHED", "CANCELLED", "FAILED"):
                return job["status"] if job else "FINISHED"
            time.sleep(1)


def read_key(creds):
    try:
        with open(creds, encoding="utf-8") as f:
            for line in f:
                m = re.match(r"^api key: *(.*)$", line.rstrip("\n"))
                if m:
                    return m.group(1).strip()
    except OSError as e:
        die(f"can't read the creds file ({e.strerror})")
    die("no 'api key:' line in the creds file")


def production_sample():
    """(source, zip member or None, extension) for the images to copy: a random spread, some
    animated, the largest. An image in a zip is `<zip path>/<member>`."""
    prod = Stash(PRODUCTION)
    images = prod.gql(
        "{ findImages(filter: {per_page: -1}) { images { id visual_files { __typename "
        "... on ImageFile { path format width height zip_file { path } } } } } }"
    )["findImages"]["images"]
    files = []
    for im in images:
        f = (im["visual_files"] or [None])[0]
        if not f or f["__typename"] != "ImageFile":
            continue
        zip_path = (f.get("zip_file") or {}).get("path")
        if zip_path and f["path"].startswith(zip_path + "/"):
            f["source"], f["member"] = zip_path, f["path"][len(zip_path) + 1:]
        elif zip_path:
            continue
        else:
            f["source"], f["member"] = f["path"], None
        files.append(f)
    rng = random.Random(12)
    by_format = {}
    for f in files:
        by_format.setdefault(f["format"], []).append(f)
    chosen = {}
    largest = sorted(files, key=lambda f: f["width"] * f["height"], reverse=True)[:LARGEST]
    for f in largest:
        chosen[f["path"]] = f
    for fmt in ("gif", "webp"):
        pool = by_format.get(fmt, [])
        for f in rng.sample(pool, min(ANIMATED_EACH, len(pool))):
            chosen[f["path"]] = f
    ordinary = [f for f in files if f["format"] in ("jpeg", "png") and f["path"] not in chosen]
    for f in rng.sample(ordinary, min(ORDINARY_COUNT, len(ordinary))):
        chosen[f["path"]] = f
    out = []
    for f in sorted(chosen.values(), key=lambda f: f["path"]):
        ext = os.path.splitext(f["path"])[1].lower() or "." + f["format"]
        out.append((f["source"], f["member"], ext))
    return out


def copy_sample(sample, dest):
    os.makedirs(dest, exist_ok=True)
    copied = missing = 0
    for n, (src, member, ext) in enumerate(sample, start=1):
        target = os.path.join(dest, f"img-{n:04d}{ext}")
        if os.path.exists(target):
            continue
        try:
            if member is None:
                shutil.copy2(src, target)
            else:
                with zipfile.ZipFile(src) as z, z.open(member) as inp, open(target, "wb") as out:
                    shutil.copyfileobj(inp, out)
            copied += 1
        except (OSError, KeyError, zipfile.BadZipFile):
            missing += 1
    return copied, missing


def edge_cases(images_dir, clips_dir):
    """The two cases Production lacks: a truncated JPEG and a short video for an image clip."""
    os.makedirs(images_dir, exist_ok=True)
    os.makedirs(clips_dir, exist_ok=True)
    corrupt = os.path.join(images_dir, "corrupt.jpg")
    if not os.path.exists(corrupt):
        full = corrupt + ".full.jpg"
        subprocess.run(["magick", "-size", "2000x1500", "plasma:fractal", full], check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        with open(full, "rb") as f:
            data = f.read()
        with open(corrupt, "wb") as f:
            f.write(data[: len(data) // 5])
        os.remove(full)
    clip = os.path.join(clips_dir, "clip-1.mp4")
    if not os.path.exists(clip):
        subprocess.run(
            ["ffmpeg", "-y", "-loglevel", "error", "-f", "lavfi", "-i",
             "testsrc2=size=640x360:rate=25:duration=5", "-pix_fmt", "yuv420p", "-c:v", "libx264", clip],
            check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--creds", required=True)
    args = ap.parse_args()
    stash = Stash(TESTING, read_key(args.creds))

    general = stash.gql(
        "{ configuration { general { stashes { path excludeVideo excludeImage } } } }"
    )["configuration"]["general"]
    libraries = [s for s in general["stashes"] if not s["excludeImage"]]
    if not libraries:
        die("Testing has no library that allows images")
    library = libraries[0]["path"]
    prod_dir = os.path.join(library, "ssv-prod")
    images_dir = os.path.join(library, "ssv-images")
    clips_dir = os.path.join(os.path.dirname(library.rstrip("/")), "ssv-clips")

    sample = production_sample()
    copied, missing = copy_sample(sample, prod_dir)
    print(f"sample {len(sample)} from Production: {copied} copied now, {missing} unreadable")
    edge_cases(images_dir, clips_dir)

    def find(fragment):
        return stash.gql(
            "query($f: String!) { findImages(filter: {per_page: -1, sort: \"path\", direction: ASC}, "
            "image_filter: {path: {value: $f, modifier: INCLUDES}}) { images { id visual_files { __typename "
            "... on ImageFile { basename } } } } }",
            {"f": fragment},
        )["findImages"]["images"]

    # An earlier generated set: only the truncated JPEG stays (Testing only; its files go too).
    generated = [i["id"] for i in find("ssv-images") if i["visual_files"][0].get("basename") != "corrupt.jpg"]
    if generated:
        stash.gql(
            "mutation($i: [ID!]!) { imagesDestroy(input: {ids: $i, delete_file: true, delete_generated: true}) }",
            {"i": generated},
        )
        for name in os.listdir(images_dir) if os.path.isdir(images_dir) else []:
            path = os.path.join(images_dir, name)
            if name != "corrupt.jpg":
                if os.path.isdir(path):
                    shutil.rmtree(path, ignore_errors=True)
                else:
                    os.remove(path)
        print(f"removed {len(generated)} generated images")

    # The clips folder as a library with videos excluded, and image clips on (Stash's rule: video
    # files in a video-excluded library become image clips).
    stashes = [
        {"path": s["path"], "excludeVideo": s["excludeVideo"], "excludeImage": s["excludeImage"]}
        for s in general["stashes"]
    ]
    if not any(s["path"].rstrip("/") == clips_dir for s in stashes):
        stashes.append({"path": clips_dir, "excludeVideo": True, "excludeImage": False})
    stash.gql(
        "mutation($s: [StashConfigInput!]) { configureGeneral(input: {stashes: $s, "
        "createImageClipsFromVideos: true}) { createImageClipsFromVideos } }",
        {"s": stashes},
    )

    job = stash.gql(
        "mutation($p: [String!]) { metadataScan(input: {paths: $p, scanGenerateThumbnails: true, "
        "scanGenerateClipPreviews: true}) }",
        {"p": [prod_dir, images_dir, clips_dir]},
    )["metadataScan"]
    print(f"scan job {job}: {stash.wait_job(job)}")

    copies = [i["id"] for i in find("ssv-prod")]
    clips = [i["id"] for i in find("ssv-clips") if i["visual_files"][0]["__typename"] == "VideoFile"]
    corrupt = [i["id"] for i in find("ssv-images")]
    print(f"copies: {len(copies)}; image clips: {len(clips)}; truncated: {len(corrupt)}")

    plan = [
        (copies[0:30], {"performer_ids": {"ids": [PERFORMER_A], "mode": "ADD"}}),
        (copies[20:40], {"performer_ids": {"ids": [PERFORMER_B], "mode": "ADD"}}),
        (copies[0:50], {"tag_ids": {"ids": [ORDINARY_TAG], "mode": "ADD"}}),
    ]
    for chosen, change in plan:
        if chosen:
            stash.gql(
                "mutation($i: BulkImageUpdateInput!) { bulkImageUpdate(input: $i) { id } }",
                {"i": {"ids": chosen, **change}},
            )
    print("performer A on the first 30 copies, performer B on copies 21-40, tag 6 on the first 50")

    existing = stash.gql("{ findSavedFilters(mode: IMAGES) { id name } }")["findSavedFilters"]
    name = "SSV images performer A"
    found = next((f for f in existing if f["name"] == name), None)
    saved = stash.gql(
        "mutation($i: SaveFilterInput!) { saveFilter(input: $i) { id } }",
        {
            "i": {
                **({"id": found["id"]} if found else {}),
                "mode": "IMAGES",
                "name": name,
                "find_filter": {"per_page": 40, "sort": "path", "direction": "ASC"},
                "object_filter": {
                    "performers": {
                        "modifier": "INCLUDES_ALL",
                        "value": {"excluded": [], "items": [{"id": PERFORMER_A, "label": "SSV Test Performer A"}]},
                    }
                },
                "ui_options": {"display_mode": 0, "zoom_index": 1},
            }
        },
    )["saveFilter"]["id"]
    print(f"saved image filter id {saved}")

    roles = stash.gql(
        "{ big: findImages(filter: {per_page: 3, sort: \"resolution\", direction: DESC}) { images { id } } "
        "gif: findImages(filter: {per_page: 3}, image_filter: {path: {value: \".gif\", modifier: INCLUDES}}) { images { id } } "
        "webp: findImages(filter: {per_page: 3}, image_filter: {path: {value: \".webp\", modifier: INCLUDES}}) { images { id } } "
        "all: findImages(filter: {per_page: 1}) { count } }"
    )
    print("ids: " + json.dumps({
        "largest": [i["id"] for i in roles["big"]["images"]],
        "gif": [i["id"] for i in roles["gif"]["images"]],
        "webp": [i["id"] for i in roles["webp"]["images"]],
        "truncated": corrupt,
        "clips": clips,
    }))
    print(f"images on Testing: {roles['all']['count']}")


if __name__ == "__main__":
    main()
