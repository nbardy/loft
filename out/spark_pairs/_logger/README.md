# spark intermediate-render logger

Shared JSONL log used by all spark agents. One JSON object per line in
[`logger.json`](logger.json); field rules in [`schema.json`](schema.json).

## Schema (one row)

| field | type | required | meaning |
|---|---|---|---|
| `ts` | string (ISO-8601 UTC) | yes | when the render was made |
| `agent` | string | yes | agent/lane id that made it |
| `prompt` | string | yes | full generator prompt |
| `seed` | integer or null | no | generator seed; `null` when backend-assigned |
| `params` | object | no | backend/size/steps/guidance, anything free-form |
| `render_path` | string | yes | repo-relative path to the render image |
| `model_path` | string or null | no | repo-relative path to GLB/FBX/USDZ; `null` when image-only |
| `note` | string | no | what this render is for |

No unknown keys. Paths are repo-relative (e.g. `out/spark_pairs/...`), never absolute.

## How to append one row

From the repo root, with single quotes around the JSON:

```sh
printf '%s\n' '{"ts":"2026-11-25T12:56:00Z","agent":"<lane>","prompt":"<full prompt>","seed":null,"params":{},"render_path":"out/spark_pairs/<render>.png","model_path":null,"note":"<what this is>"}' >> out/spark_pairs/_logger/logger.json
python3 out/spark_pairs/_logger/validate.py
```

Log **every** intermediate render (concept image or model build): prompt, seed,
params, and the real `render_path`/`model_path`. Prefer copying shared renders
into `out/spark_pairs/` so the logged path stays inside the repo.

## How to validate (one command)

```sh
python3 out/spark_pairs/_logger/validate.py
```

It checks JSON-per-line, required keys/types, no unknown keys, and that every
logged `render_path` (and non-null `model_path`) exists on disk. Exit `0` with
`VALID: N rows ...` on success.
