"""Compare original decoded frames, hash duplicates once, and save amplified lossless diffs."""
import json
from pathlib import Path
import sys

import numpy as np
from PIL import Image

root, destination = map(Path, sys.argv[1:])
destination.mkdir(exist_ok=True, parents=True)
scenes = dict(ball='ball', sun='ball_sunonly', base='otter', walrus='tq',
              heron='h02_hero_q3', otter='hero_q3')


def pixels(path):
    with Image.open(path) as image:
        return np.asarray(image.convert('RGBA'), dtype=np.int16)


results = {}
for phase in ('candidate', 'guard', 'bounds', 'repeat'):
    for size in ('960x720', '1280x800'):
        for name, stem in scenes.items():
            hashes = f'{stem}-{size}-hashes.json'
            original = root/'baseline'/hashes
            if not original.exists():
                continue
            before = [root/'baseline'/(h+'.png') for h in json.loads(original.read_text())]
            other = root/phase/hashes
            if other.exists():
                after = [root/phase/(h+'.png') for h in json.loads(other.read_text())]
                assert len(before) == len(after) == 120
            else:
                clip = root/phase/f'{name}-{size}-clip'
                if not clip.is_dir():
                    continue
                assert sorted(p.name for p in clip.glob('frame_*.png')) == [f'frame_{i:03}.png' for i in range(3)]
                after = [clip/f'frame_{i:03}.png' for i in range(3)]
                before = before[:3]
            pairs = list(zip(before, after))
            stats = {}
            worst = None
            for pair in set(pairs):
                if pair[0].name == pair[1].name:  # hash-named files contain the exact decoded RGBA digest
                    stats[pair] = (0,0,0)
                    continue
                dif = np.abs(pixels(pair[0])-pixels(pair[1]))
                result = int(dif.max()), int(np.count_nonzero(np.any(dif != 0,axis=2))), int(dif.sum())
                stats[pair] = result
                if result[0] and (worst is None or result > worst[0]):
                    worst = result,dif
            key = '/'.join([phase,size,name])
            maxima = tuple(map(max,zip(*stats.values())))
            written = None
            if worst is not None:
                # Show even a one-unit alpha difference.
                rgb = np.maximum(worst[1][:,:,:3],worst[1][:,:,3,None])
                written = f'{phase}-{size}-{name}-diff.png'
                Image.fromarray(np.minimum(255,rgb*16).astype('uint8')).save(destination/written)
            original_names = {b.name for b in before}
            source_members = sum(p.name in original_names for p in after) if other.exists() else None
            results[key] = dict(frames=len(pairs), source_member_frames=source_members,
                                exact=sum(stats[pair][0]==0 for pair in pairs),
                                max_rgba8=maxima[0], max_pixels_per_frame=maxima[1], diff=written)
print(json.dumps(results,indent=2,sort_keys=True))
