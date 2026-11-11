# Design variants

Written before the code (2026-11-11), then scored against it (last section). The code is `src/design.ts` (the
notion), `src/cli/variant.ts` and `src/cli/variants.ts` (choosing one, and the comparison sheet), and the crab:
`examples/bosses/reefback_titan.ts` (its design) over `examples/bosses/titan/` (its rig and slots).

## What a variant is

A Loft monster is a set of closed-form functions of the clock. Some of those functions are what the game reads and
what the checks measure: where the body is, when an attack arms, where a strike lands, what a grip holds and where.
The rest are what an art director argues about: the shape of a leg, the face, what grows on the back. A variant
changes the second set and must leave the first set *valid*, not unchanged.

So a **design** (one subject, here the crab) has four parts:

- **numbers**: one record of every number an art director may change (inks, sizes, timings);
- a **rig**: everything the game reads and the checks measure: the walk, the body and shell frames, each attack's
  clock and target, the held ship, the warnings, the crash, and the *mounts* where slots attach (leg roots, claw
  shoulders, the shell frame);
- **slots**: legs, claws, face, back, carapace. Each slot has named **options**; an option is a function from the
  rig (and the numbers) to drawables, plus whatever parts the checks need from it;
- **checks**: the contract. They are built from the rig's attacks and from parts the slots return (the crusher's palm
  that strikes, the cutter's two fingers that grip).

A **variant** is one option per slot plus number overrides. It is valid when it builds and passes the subject's checks.
That is the whole definition: there is no list of things a variant may not touch. A stilt-legged crab that raises the
body is valid if the Claw Crush still lands inside its warning and the cutter still closes on the ship's beam; if it
does not, the check says by how much, which is a geometric fact about that crab, not a policy.

What stays fixed across the variants of one subject: the slot names, the rig's *code* (its attacks, their names, what
the game reads), the camera set and the checks. What may change: any option in any slot, any number. Numbers include
proportions, so a variant may change proportions; the checks are re-measured on each variant, never inherited.

## Slots that ask things of the rig

The rig cannot know every option. Two directions of dependency appear in the crab, and they get two mechanisms.

1. **An option needs different numbers** (stilt legs need a higher body; a fiddler wants one claw three times the
   other). The option carries `numbers`, overrides on the subject's record. They apply to the whole build, so the rig
   reads them like any number. Two chosen options that set one number to different values are a typed error naming
   both; a variant's own `numbers` are applied last, over the options' (the art director's word wins, and the checks
   still judge it).
2. **The rig's attack geometry depends on the option's shape.** The Grab places the ship where the cutter can reach it
   closed on the beam: that distance is wrist reach plus *how deep the grip's middle sits beyond the wrist when the
   fingers close on a thing of width w*, which depends on the fingers' length, curl and hinge spacing. Claw Crush lands
   the wrist half a palm short of the disc's centre. So the claws slot **offers** the rig a small typed interface
   computed from the numbers alone (plain JavaScript, before the rig exists): `crushLead` and a `Gripper`
   (`closedDepth(width)`, `restGap`). The rig solves its geometry against the interface; the option draws against the
   rig. The order is acyclic: numbers, then offers, then rig, then drawings, then checks.

   A fortress crab whose claws fold over the face needs nothing new: the rest pose is the claws option's own, and the
   crush's lunge law starts from whatever rest the option gives it. A fiddler is mechanism 1 (`crusher: 3`) plus
   mechanism 2 (a bigger palm offers a longer lead).

This is the seam test: a new option that forces an edit to the rig or to another slot means the interface is missing a
field. Add the field to the slot's contract, not a case to the rig.

## Naming, combining, showing

- Options are named per slot (`legs: base | stilt`). Every design has a variant named `base` that picks an option in
  every slot; every other named variant is a partial pick over `base` plus numbers.
- A choice is written `name` or `name,slot=option,...` or `slot=option,...` (over `base`): `--variant stilt`,
  `--variant legs=stilt,face=mask`, `--variant stilt,face=mask`. It is parsed once, at the CLI's boundary, into
  `{ variant, picks }`, and resolved once against the design; an unknown variant, slot or option is a `DesignError`
  that lists the valid ones. Nothing downstream looks at text.
- A module that exports a design also exports `default` (the base variant's scene), `checks`, `strikes` and `holds`
  of the base variant, so everything that reads a plain scene module still works unchanged.
- `loft variants <scene>` renders every named variant (rows) through the design's comparison cameras (columns), writes
  each picture, and writes one labelled sheet. `loft check <scene>` with no `--variant` checks every named variant.

## Inherited and new

Inherited: options as a choice per slot is the obvious shape of any configurator. New here: the checks are the
definition of validity (no whitelist of allowed changes), the rig is defined by what the game reads and the checks
measure rather than by a skeleton, and options talk to the rig through *offers*: small measured interfaces computed
before the rig, so the attack geometry is solved for the chosen shape rather than tuned for one.

## Predictions (written before the code)

Files (lines approximate):

| file | lines | what |
|---|---|---|
| `examples/bosses/reefback_titan.ts` | ~90 | the design: the slot table, the named variants, the cameras, the assembly into a scene and its checks |
| `titan/numbers.ts` | ~100 | `S` moved verbatim, and its type |
| `titan/ink.ts` | ~20 | the shared inks and the two pattern helpers (`cut`, `mottle`) |
| `titan/slots.ts` | ~80 | each slot's contract: what an option receives and returns, and the claws' offer |
| `titan/rig.ts` | ~220 | walk, Claw Crush, sea, Grab, the ship, frames, mounts, warnings, crash |
| `titan/carapace/base.ts` | ~25 | |
| `titan/legs/base.ts` | ~35 | |
| `titan/claws/base.ts` | ~130 | |
| `titan/face/base.ts` | ~215 | |
| `titan/back/base.ts` | ~165 | atoll, reef, tower |

A new leg design: one file of 25 to 40 lines plus two lines in the design (import, entry). A face or back: one file,
from 20 lines (reusing the base's parts) to the base's 200; plus two lines.

Pictures: the base variant renders bit-identical frames to main for `game`, `three`, `close` at t = 1.0, 3.6, 11.3:
the same expressions, built in the same order, drawn in the same order. Checks: base passes the same 4.
