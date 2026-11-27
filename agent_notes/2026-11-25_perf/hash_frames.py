"""Hash the actual RGBA bytes of every frame, including the first unretained frame."""
from hashlib import sha256
from io import BytesIO
import json
from pathlib import Path
import shutil
import subprocess
import sys
from tempfile import TemporaryDirectory

from PIL import Image

root, phase, size, *scenes = sys.argv[1:]
root = Path(root)
exe = root/phase/'loft'
fixtures = root/phase/'source/fixtures' if (root/phase/'source/fixtures').is_dir() else root/'fixtures'
for scene in scenes:
    name = Path(scene).stem
    folder = root/phase
    output = folder/f'{name}-{size}-hashes.json'
    if output.exists():
        continue
    with TemporaryDirectory(dir=folder) as temp:
        clip = Path(temp)/'clip'
        subprocess.run([exe,'clip',fixtures/scene,'--frames','120','--fps','60','--size',size,'--out',clip],check=True)
        hashes = []
        decoded = {}
        for i in range(120):
            frame = clip/f'frame_{i:03}.png'
            encoded = frame.read_bytes()
            png_digest = sha256(encoded).digest()
            # Identical PNG bytes decode identically. Preserve the RGBA hash convention
            # and only expand a distinct compressed frame once per clip.
            if png_digest not in decoded:
                with Image.open(BytesIO(encoded)) as img:
                    decoded[png_digest] = sha256(img.convert('RGBA').tobytes()).hexdigest()
            hashes.append(decoded[png_digest])
            # Keep one lossless representative of each distinct result for pixel diffs.
            sample = folder/f'{hashes[-1]}.png'
            if not sample.exists():
                shutil.copyfile(frame,sample)
        output.write_text(json.dumps(hashes))
        print(name, size, len(hashes), 'frames;', len(set(hashes)), 'unique', flush=True)
