"""Score all completed attempts without selecting by frame time. Run with uv run python."""
import json
import math
import re
import sys
from functools import cache
from pathlib import Path
from statistics import median

root = Path(sys.argv[1])
scenes = dict(ball='ball', sun='ball_sunonly', base='otter', walrus='tq',
              heron='h02_hero_q3', otter='hero_q3')


def ranked(values, p):
    return sorted(values)[math.ceil(p * len(values)) - 1]


def read(path):
    return json.loads(path.read_text())


def profile(path, mode=None):
    d = read(path)
    if mode:
        d = d[mode]
    times = d['gpu_frame_ms']
    mid = ranked(times, .5)
    passes = d['gpu_pass_ms']
    return dict(p50=mid, p95=ranked(times, .95),
                pass_in_median_frame=passes[times.index(mid)],
                vis=ranked([row[1] for row in passes], .5),
                vis_nonzero=sum(row[1] > 0 for row in passes),
                brackets=[d['load_control_before_p50_ms'], d['load_control_after_p50_ms']],
                contended=d['contended'], frames=len(times), source=path.name)


def roots_per_field(wgsl):
    """Expanded syntactic length, distance and square-root count. Constant loops unrolled.
    This counts even both sides of conditionals and does not represent retired instructions.
    """
    functions = {}
    for m in re.finditer(r'\bfn (\w+)\([^{}]*\)[^{}]*\{', wgsl):
        level = 1
        for brace in re.finditer(r'[{}]', wgsl[m.end():]):
            level += 1 if brace[0] == '{' else -1
            if level == 0:
                functions[m[1]] = wgsl[m.end():m.end()+brace.start()]
                break

    @cache
    def count(name):
        result, weight, scopes = 0, 1, []
        for line in functions[name].splitlines():
            line = line.partition('//')[0]
            for tok in re.finditer(r'[{}]|\b(\w+)\(', line):
                if tok[0] == '}':
                    weight //= scopes.pop()
                elif tok[0] == '{':
                    loop = re.search(r'for \(var \w+: u32 = 0u; \w+ < (\d+)u;', line[:tok.start()])
                    mult = int(loop[1]) if loop else 1
                    scopes.append(mult)
                    weight *= mult
                elif tok[1] in ('length', 'distance', 'sqrt', 'inverseSqrt'):
                    result += weight
                elif tok[1] in functions:
                    result += weight * count(tok[1])
        return result
    return count('q_trace')


attempts = {}
selected = {}
for phase in ('baseline', 'candidate', 'cal', 'guard', 'bounds', 'bounds2'):
    for size in ('960x720', '1280x800', '672x504'):
        for scene in scenes:
            paths = sorted((root/phase).glob(f'{scene}-{size}-[0-9]*.json'))
            if not paths:
                continue
            options = [profile(path) for path in paths]
            key = '/'.join([phase, size, scene])
            attempts[key] = options
            selected[key] = next((x for x in options if not x['contended']), options[0])

models = {}
pooled_empty = median(read(p)['cal_empty_ms'] for p in (root/'model').glob('*.counts.json'))
for name in scenes.keys() - {'sun'}:
    path = root/'model'/f'{name}.counts.json'
    if not path.exists():
        continue
    counts = read(path)
    modules = {m['name']: m['wgsl'] for m in read(root/'model'/f'{name}.show')['modules']}
    empty = pooled_empty
    raw_first = counts['cal_empty_ms']
    calwork = 0.
    caltraced = 0.
    ideal = 0.
    ideal_field = 0.
    work = dict(trace_miss=0., trace_hit=0., analytic_miss=0., analytic_hit=0.)
    contribution = {}
    upper = dict(bounds=0., thin=0., field_prune=0.)
    rates = {}
    totals = [0] * 5
    visible = 0
    root_calls = 0
    for kind, data in counts['kinds'].items():
        p, h, sh, sm, cap = data['target']
        pc, hc, shc, smc, _ = data['calibration']
        traced = data['draw'] == 'Traced'
        ms = median(data['calibration_four_draw_ms'])
        cost = (ms - empty)/4
        scale_to_target = (sh + sm)/(shc + smc) if traced else p/pc
        raw_first += (ms-counts['cal_empty_ms'])*scale_to_target/4
        rate = max(0., cost) / (shc + smc if traced else pc)
        if traced:
            caltraced += cost
            ideal_field += rate * data['shaded']
            work['trace_miss'] += sm * rate
            work['trace_hit'] += sh * rate
        else:
            work['analytic_miss'] += (p - h) * rate
            work['analytic_hit'] += h * rate
        root_count = roots_per_field(modules[kind]) if traced else 0
        root_calls += root_count * (sh + sm)
        visible += data['shaded']
        totals = [a+b for a,b in zip(totals, data['target'])]
        ideal += rate * data['shaded']
        calwork += cost
        short = kind.split('_')[-1]
        contribution[short] = rate * (sh + sm if traced else p)
        if short in {'Otter', 'Hide', 'Cap', 'HeronHead', 'Head4', 'Torso'}:
            upper['bounds'] += .9 * sm * rate
        if short in {'Rod', 'Strand', 'Whisker'}:
            upper['thin'] += max(0., p - 4*h - 36*data['rows']) * rate
        if traced and root_count >= 8:
            upper['field_prune'] += .5 * (sh + sm) * rate
        rates[short] = dict(ns_per_work=rate*1e6, roots_per_field=root_count,
                            hidden_ratio=median(data['calibration_occluded_four_ms'])/ms)
    target_work = sum(work.values())
    orig = selected['baseline/960x720/'+name]
    observed = orig['pass_in_median_frame']
    cal = selected.get('cal/672x504/'+name)
    correction = (cal['vis'] - empty)/calwork if cal else None
    models[name] = dict(raw=raw_first, pooled_raw=empty+target_work, normalized=empty+correction*target_work if cal else None,
                        cal_ms=cal['vis'] if cal else None, scale=correction,
                        work_cal=[caltraced, calwork-caltraced],
                        work_target=[work['trace_hit']+work['trace_miss'],work['analytic_hit']+work['analytic_miss']],
                        actual=orig['vis'], totals=totals, shaded=visible,
                        field_root_calls=root_calls, rates=rates,
                        budget={k:(observed[1]-empty)*v/target_work for k,v in work.items()},
                        raster_clear_estimate=empty, non_visibility=orig['p50']-observed[1],
                        one_query_frame=orig['p50']-observed[1]+empty+ideal*(correction if cal else 1.),
                        eight_query_frame=orig['p50']-observed[1]+empty+(ideal+7*ideal_field)*(correction if cal else 1.),
                        opportunities_ms={k:(observed[1]-empty)*v/target_work for k,v in upper.items()},
                        kinds={k:(observed[1]-empty)*v/target_work for k,v in contribution.items()})
# Two-column nonnegative least squares. Include both boundary solutions.
shared = {}
if len(models) == 5 and all(m['cal_ms'] is not None for m in models.values()):
    rows = [[x/(m['cal_ms']-m['raster_clear_estimate']) for x in m['work_cal']] for m in models.values()]
    aa = sum(a*a for a,b in rows)
    ab = sum(a*b for a,b in rows)
    bb = sum(b*b for a,b in rows)
    ay, by = [sum(col) for col in zip(*rows)]
    det = aa*bb-ab*ab
    candidates = [(ay/aa,0.), (0.,by/bb)]
    if det > 0:
        candidates.append(((ay*bb-by*ab)/det,(by*aa-ay*ab)/det))
    fit = min((x for x in candidates if min(x)>=0),
              key=lambda x:sum((a*x[0]+b*x[1]-1.)**2 for a,b in rows))
    shared['multipliers'] = fit
    for m in models.values():
        m['shared_prediction'] = m['raster_clear_estimate'] + sum(a*b for a,b in zip(fit,m['work_target']))
paired_ring = {}
for path in sorted(root.glob('*/*-ab.json')):
    phase = path.parent.name
    scene, size, _ = path.stem.split('-')
    paired_ring['/'.join([phase,size,scene])] = profile(path, 'ring')
hashes = {}
for size in ('960x720', '1280x800'):
    for name, stem in scenes.items():
        filename = f'{stem}-{size}-hashes.json'
        pair = [root/phase/filename for phase in ('baseline','candidate')]
        if all(p.exists() for p in pair):
            before, after = map(read,pair)
            hashes[size+'/'+name] = dict(frames=len(before), exact=sum(a==b for a,b in zip(before,after)),
                                         unique_before=len(set(before)), unique_after=len(set(after)))
print(json.dumps(dict(selected=selected, attempts=attempts, models=models, shared=shared, hashes=hashes, paired_ring=paired_ring,
                      diagnostics=read(root/'aov-summary.json')), indent=2, sort_keys=True))
