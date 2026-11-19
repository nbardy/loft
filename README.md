# Loft v1.0

Loft is a GPU meta engine for 3D rendering and motion. Author in `.loft` with libraries in [lib/](lib/) and render schedules in [engines/](engines/). The Rust workspace in [crates/](crates/) compiles and runs them on wgpu.

Run `tools/dev.sh run loft-cli -- check engines/hermit.loft` to check a scene and `tools/check.sh quick` to check changes. See the [v1.0 specification](docs/spec/) and [legacy source archive](DEPRECATED.md).
