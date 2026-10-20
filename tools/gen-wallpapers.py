#!/usr/bin/env python3
"""Generates the app's built-in wallpapers with OpenAI's image API.

Writes ui/wallpapers/<category>/<n>.jpg (2560x1440), ui/wallpapers/thumbs/<category>/<n>.jpg
(320x180) and ui/wallpapers/manifest.js. Already-generated images are kept, so a rerun only
fills gaps. Needs ffmpeg, and OPENAI_API_KEY in the environment or in
~/.config/precap/openai.env (a line OPENAI_API_KEY=...).

  python3 tools/gen-wallpapers.py [--quality low|medium|high] [--model MODEL] [--only dark]
"""
import argparse, base64, json, os, pathlib, subprocess, sys, urllib.error, urllib.request
from concurrent.futures import ThreadPoolExecutor

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "ui" / "wallpapers"
RAW = ROOT / "target" / "wallpapers-raw"  # the API's originals, so reruns don't pay again
STYLE = (", original artwork, high detail, smooth, no text, no letters, no logos, no watermark, "
         "no people, wide desktop wallpaper composition")

CATEGORIES = [
    ("landscapes", "Landscapes", [
        "Serene mountain lake at dawn, soft pastel sky, mist over the water, minimalist painterly digital art",
        "Rolling desert dunes at golden hour, long soft shadows, warm sand tones, minimalist",
        "Coastal cliffs above a turquoise sea, gentle clouds, bright daylight, clean stylized illustration",
        "Snowy alpine valley under a starry night sky, deep blues, calm and quiet",
        "Green rolling hills with soft layers of morning fog, gradient sky, minimalist landscape",
        "Tropical lagoon seen from above, aqua water and white sand patterns, aerial view",
    ]),
    ("dark", "Dark", [
        "Deep charcoal abstract waves with subtle glossy highlights, dark minimal wallpaper",
        "Black background with faint flowing ribbons of graphite-gray smoke",
        "Dark gradient from midnight navy to black with soft grain and a faint glow at one edge",
        "Matte black folded paper shapes lit softly from one side",
        "Dark abstract liquid metal swirls with subtle silver reflections",
        "Very dark out-of-focus bokeh lights in muted colors at night",
    ]),
    ("blue", "Blue", [
        "Flowing translucent blue glass ribbons on a soft light-blue background",
        "Deep ocean-blue gradient with soft caustic light patterns",
        "Layered blue paper-cut waves with gentle shadows, calm",
        "Smooth blue and cyan aurora gradient, soft and minimal",
        "Abstract blue fluid paint marbling with elegant swirls",
        "Soft blue sky with dreamy pastel clouds, minimalist",
    ]),
    ("purple", "Purple", [
        "Purple and magenta smooth gradient mesh with soft glowing orbs",
        "Violet translucent glass shapes floating in soft studio lighting",
        "Lavender and indigo layered abstract dunes, minimal",
        "Soft purple nebula clouds with a few gentle stars, calm",
        "Ultraviolet flowing silk fabric folds with smooth highlights",
        "Pastel lilac to deep plum gradient with subtle grain",
    ]),
    ("orange", "Orange", [
        "Warm orange and peach gradient with a soft sunlight flare",
        "Abstract glossy orange fluid waves, energetic but clean",
        "Sunset over layered desert mesas in orange and red tones, minimalist illustration",
        "Tangerine and pink paper-cut layers with soft shadows",
        "Glowing amber liquid shapes on a dark warm background",
        "Soft apricot sky with golden clouds, dreamy",
    ]),
    ("cities", "Cities", [
        "Futuristic city skyline at dusk under a soft purple and orange sky, stylized illustration",
        "Rainy city street at night with blurred neon reflections, cinematic, no readable signs",
        "Aerial view of a modern city grid at blue hour with glowing lights",
        "Mediterranean hillside town with white houses above a blue sea, sunny, painterly",
        "Minimalist city skyline silhouette against a pastel sunrise gradient",
        "Snowy old-town rooftops at twilight with warm lit windows, calm illustration",
    ]),
]


def api_key():
    key = os.environ.get("OPENAI_API_KEY")
    env = pathlib.Path.home() / ".config" / "precap" / "openai.env"
    if not key and env.exists():
        for line in env.read_text().splitlines():
            if line.startswith("OPENAI_API_KEY="):
                key = line.split("=", 1)[1].strip().strip("'\"")
    if not key:
        sys.exit("no OPENAI_API_KEY (set it, or put OPENAI_API_KEY=... in ~/.config/precap/openai.env)")
    return key


def request(key, path, body=None):
    req = urllib.request.Request("https://api.openai.com/v1/" + path, data=json.dumps(body).encode() if body else None,
                                 headers={"Authorization": "Bearer " + key, "Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=300) as r:
            return json.load(r)
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"{path}: HTTP {e.code}: {e.read().decode(errors='replace')[:400]}") from None


def pick_model(key):
    """The newest gpt-image model this key can use."""
    ids = [m["id"] for m in request(key, "models")["data"] if m["id"].startswith("gpt-image")]
    if not ids:
        sys.exit("this API key has no gpt-image model")

    def rank(i):  # gpt-image-2 > gpt-image-1.5 > gpt-image-1 > gpt-image-1-mini
        parts = i.split("-")
        try:
            v = float(parts[2])
        except (IndexError, ValueError):
            v = 0.0
        return (v, "mini" not in i, -len(i))
    return max(ids, key=rank)


def ffmpeg(*args):
    subprocess.run(["ffmpeg", "-v", "error", "-y", *args], check=True)


def make(key, model, quality, cat, n, prompt):
    raw = RAW / cat / f"{n}.png"
    if not raw.exists():
        data = request(key, "images/generations", {"model": model, "prompt": prompt + STYLE, "size": "1536x1024", "quality": quality, "n": 1})
        raw.parent.mkdir(parents=True, exist_ok=True)
        raw.write_bytes(base64.b64decode(data["data"][0]["b64_json"]))
    full, thumb = OUT / cat / f"{n}.jpg", OUT / "thumbs" / cat / f"{n}.jpg"
    for f in (full, thumb):
        f.parent.mkdir(parents=True, exist_ok=True)
    # Centre-crop to 16:9, then size for a 1440p frame and a picker thumbnail.
    crop = "crop=iw:iw*9/16"
    ffmpeg("-i", str(raw), "-vf", f"{crop},scale=2560:1440:flags=lanczos", "-q:v", "3", str(full))
    ffmpeg("-i", str(raw), "-vf", f"{crop},scale=320:180:flags=lanczos", "-q:v", "4", str(thumb))
    return f"{cat}/{n}"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--quality", default="medium", choices=["low", "medium", "high"])
    ap.add_argument("--model")
    ap.add_argument("--only")
    args = ap.parse_args()
    key = api_key()
    model = args.model or pick_model(key)
    print(f"model {model}, quality {args.quality}")
    jobs = [(cat, n, p) for cat, _, prompts in CATEGORIES if not args.only or cat == args.only for n, p in enumerate(prompts, 1)]
    failed = []
    with ThreadPoolExecutor(4) as pool:
        for (cat, n, _), fut in zip(jobs, [pool.submit(make, key, model, args.quality, *j) for j in jobs]):
            try:
                print("ok", fut.result(), flush=True)
            except Exception as e:  # keep going; a rerun retries just the gaps
                failed.append(f"{cat}/{n}: {e}")
                print("failed", failed[-1], flush=True)
    # The manifest lists whatever exists, so the app never points at a missing file.
    cats = [{"id": cat, "name": name, "items": [f"{cat}/{n}" for n in range(1, len(prompts) + 1) if (OUT / cat / f"{n}.jpg").exists()]}
            for cat, name, prompts in CATEGORIES]
    cats = [c for c in cats if c["items"]]
    (OUT / "manifest.js").write_text("// Generated by tools/gen-wallpapers.py; built-in wallpapers by category.\nconst WALLPAPERS = "
                                     + json.dumps(cats, indent=1) + ";\n")
    print(f"{sum(len(c['items']) for c in cats)} wallpapers in manifest; {len(failed)} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
