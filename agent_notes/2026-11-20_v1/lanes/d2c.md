# D2c · Portable execution

Base `d0e6a79`. Descriptor and measurement ledger written before editing.

| Descriptor | V3 | Expected |
|---|---|---|
| Lighting storage view | offset implicit 0, size implicit allocation tail | offset 0, size 4,488 B (1,122 f32); reserved padding excluded |
| Adopted storage | implicit allocation tail | checked logical range independent of capacity; dynamic arenas deliberately bind their whole logical capacity |
| Buffer usage | blanket VERTEX / INDEX / INDIRECT | indirect and index derived from scheduled operands; no VERTEX for unbound vertex buffers |
| Classify | WGSL identifier matched after crossing the ABI boundary | original (1,1) resolves shade_args before remap, including the archived data name |
| Bake view | inferred storage format after layout creation | explicit authored format for new descriptors, inferred for legacy; preflight device, with byte-packed copy path when needed |
| Pipeline recipe | lazy serialized creation | unique misses in parallel, count/timestamp real creates including load-only work |

Predicted 18/18 exact native and archive/embed frames against retained main, including capsules-1 center HDR red 0.202026 and RGBA (150,152,60,255). Frozen Pictures stays 0/18 until owner approval. AOV/depth exact, both depth conventions, 5/5/17 commands, 20 load bake dispatches and 5/0/0/5/0 dirty work, Twin 36/36, Truth 34/34, Speed 12/12 and Load ≥31/33. Expect cold Hermit below 1 s on clean nonces, with warm and cold separate. Old round trips must be byte exact.

| Physical execution | V3 | Ceiling |
|---|---:|---:|
| Runner and declaration | 1,512 | 1,100 |
| Std | 2,429 | 1,003 |
| Std pipeline worker | 67 | 67 |
| Native CLI | 1,201 | 300 |
| Total | 5,209 | 2,470 |

V3 span ledger uses `out/meta/v3/l0.py` and `after-l0.json`. Runner: all production source and included execution declaration, excluding test-only code. Std: attachments 1–379, bindings 1–173, edits 1–242, error 1–21, frame 1–180, interop 1–363, mips 1–42 and 75–225, outbox 1–146, pick 49–232, schedule 1–314, storage 1–140, web 1–94, promotion 111–177. Native: mod 1–103 and 112–589, edit 1–360, motion 12–16, 27–35, 68–114, 128–129, ray_query 1–197. Count relocated runtime and new source at destination.

## Implementation and physical lines

The runner binds explicit byte offsets and lengths. The defaults follow declared capacity for an arena, including reserved rows, and declared storage size for other buffers. Source pass maps use original shader addresses, including Classify (1,1). The v2 execution descriptor adds byte views and explicit bake formats, with v1 reflecting the storage format and using the declared byte range. The archive codec preserves the original descriptor bytes. The shared recorder has an explicit timestamp start and supports caller-owned color, depth and encoder. The native twin and triangle timing control use ordinary declarations. The wasm entry now expects the caller’s wgpu device and queue. Conversion of a raw browser GPUDevice into a wgpu Device remains open.

The packed path sends one storage-buffer dispatch and copy per output level. It preserves the authored format, dirty clock and slice coordinates. The default-limit cube route splits the six faces across workgroups. Independent compute misses and independent raster misses each use parallel creates on native. Per-create counts use actual calls, including mip recipes. The existing adapter and nonce key identify Vulkan cache files.

The extension still relies on existing defaults for texture subresource views, sampler policy, topology and index offsets, and attachment load/store. General custom state for those fields remains open. The native edit API matches all public edit-session signatures on origin/main.

| Crate | V3 physical | D2c physical |
|---|---:|---:|
| loft-abi | 498 | 498 |
| loft-cli | 4,307 | 4,183 |
| loft-format | 815 | 815 |
| loft-ir | 548 | 720 |
| loft-parse | 2,723 | 2,723 |
| loft-passes | 16,734 | 16,627 |
| loft-plan | 949 | 949 |
| loft-runner | 1,998 | 2,387 |
| loft-std | 4,321 | 4,241 |

| Charged location | V3 | D2c | Budget |
|---|---:|---:|---:|
| Runner including declaration and relocated twin | 1,512 | 1,977 | 1,100 |
| Std including worker | 2,496 | 2,419 | 1,070 |
| Native CLI | 1,201 | 1,077 | 300 |
| Total | 5,209 | 5,473 | 2,470 |

The total misses the 2,500-line gate by 2,973. The spans below follow V3's physical convention, charging the relocated descriptor and twin lowering where they live. The largest surviving spans are Runner's prepare/record implementation, Std attachments and native frame/readback and edit handling.

| Charge | File under crates/ | Physical spans | Lines |
|---|---|---|---:|
| runner | `loft-runner/src/backend.rs` | 1–79 | 79 |
| runner | `loft-runner/src/bundle.rs` | 1–14 | 14 |
| runner | `loft-runner/src/command.rs` | 1–129 | 129 |
| runner | `loft-runner/src/embed.rs` | 1–128 | 128 |
| runner | `loft-runner/src/encode.rs` | 1–557 | 557 |
| runner | `loft-runner/src/events.rs` | 1–12 | 12 |
| runner | `loft-runner/src/frame.rs` | 1–53 | 53 |
| runner | `loft-runner/src/initialize.rs` | 1–164 | 164 |
| runner | `loft-runner/src/interop.rs` | 1–63 | 63 |
| runner | `loft-runner/src/lib.rs` | 1–223 | 223 |
| runner | `loft-runner/src/mips.rs` | 1–148 | 148 |
| runner | `loft-runner/src/params.rs` | 1–15 | 15 |
| runner | `loft-runner/src/pick.rs` | 1–32 | 32 |
| runner | `loft-runner/src/resources.rs` | 1–23 | 23 |
| runner | `loft-runner/src/stepping.rs` | 1–132 | 132 |
| runner | `loft-runner/src/tier0.rs` | 1–13 | 13 |
| runner | `loft-ir/src/execution.rs` | 1–171 | 171 |
| runner | `loft-passes/src/bundle.rs` | 714–734 | 21 |
| std | `loft-std/src/attachments.rs` | 1–379 | 379 |
| std | `loft-std/src/bindings.rs` | 1–173 | 173 |
| std | `loft-std/src/edits.rs` | 1–242 | 242 |
| std | `loft-std/src/error.rs` | 1–22 | 22 |
| std | `loft-std/src/frame.rs` | 1–180 | 180 |
| std | `loft-std/src/interop.rs` | 1–374 | 374 |
| std | `loft-std/src/mips.rs` | 1–46, 78–228 | 197 |
| std | `loft-std/src/outbox.rs` | 1–146 | 146 |
| std | `loft-std/src/pick.rs` | 49–232 | 184 |
| std | `loft-std/src/schedule.rs` | 1–314 | 314 |
| std | `loft-std/src/storage.rs` | 1–141 | 141 |
| std | `loft-std/src/promotion.rs` | 111–177 | 67 |
| native | `loft-cli/src/native/mod.rs` | 1–101, 110–487 | 479 |
| native | `loft-cli/src/native/edit.rs` | 1–360 | 360 |
| native | `loft-cli/src/native/motion.rs` | 12–16, 27–35, 68–91, 105–106 | 40 |
| native | `loft-cli/src/native/ray_query.rs` | 1–198 | 198 |

## Single build and gate status

`CARGO_BUILD_JOBS=2 tools/dev.sh build loft-cli --fast --workspace --all-targets` waited 735.3 s for the shared compile lock and exited 101. The first and only emitted compile error was E0369 at `crates/loft-runner/src/encode.rs:382:58`. The timestamp preflight used `set.ty() != wgpu::QueryType::Timestamp`, but wgpu 30.0.1 does not implement `PartialEq` for `QueryType`. The checkout uses `matches!` at that location. This one-line correction is unbuilt. No candidate GPU test executable was produced and no second build was attempted. The build log and the staged GPU probe driver are preserved in the primary checkout’s ignored `out/meta/d2c/`.

| Baseline evidence | Measured before this lane | D2c |
|---|---|---|
| V3 strict retained-main frames | 0/18 exact | Unmeasured |
| Prior one-field range probe | 18/18 exact, LUT offset 0 with 4,488 bytes and 1,122 f32s | Expected, unmeasured |
| Capsules-1 t=0 center | V3 HDR red 31.484375, halfword `0x4fdf`, white. Prior range probe HDR red 0.20202637, halfword `0x3277`, RGBA (150,152,60,255) | Unmeasured |
| Hermit timing experiment on unique nonces | 0.869/0.829 s parallel and 3.143/1.573 s serial | Unmeasured |
| Twin / Truth / Speed / Load | V3 36/36, 34/34, 12/12, 28/33 | Unmeasured |
| Frozen Pictures and owner review | V3 0/18 against frozen Pictures. The 18 layered-material frames are in the existing owner queue | No reference writes or approvals |

The two retained main frame sets in `out/meta/iv/frames` and `out/meta/iw/frames` match at all RGBA pixels on all 18 inputs. The probe driver uses the former for strict parity. GPU gates, byte-exact mip comparisons, old archive round trips, archived Classify, edits, and browser execution remain unverified on D2c. Static Rust parsing and whitespace checks passed. The 5,473-line charged span result fails the size gate independently of the compiler error.

ESCALATE
