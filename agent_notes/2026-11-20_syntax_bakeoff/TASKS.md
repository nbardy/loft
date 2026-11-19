# Loft scene tasks

You have one Loft language card. Use only what it shows. If you need a helper function the card does not list, write it
yourself in the card's syntax. Write each task as one complete file: spaces, kinds or their equivalent, the rows the
build emits, materials, any laws or constraints, the lights, a camera and a schedule. Give metres and radians with
units. Do not explain your file; only write it.

## Task 1: a hermit crab

A hermit crab sits on wet sand.

- **The shell** is a coiled gastropod spiral. Build it by scanning frames along a spiral spine: start one frame from
  the spine's tangent, then transport it along the spine so the frames do not twist. Sweep a ribbed tube around the
  spine, and grow the tube's radius with the spiral. Rasterise the shell as a parametric surface.
- **The crusher claw** is a signed distance field: a rounded palm with a fixed finger and a hinged moving finger, with
  serrations along the moving finger. Its opening angle is per-instance data. Use the same claw definition for a
  smaller feeder claw on the other side, which differs only in its data.
- **The twelve legs** (six per side) are one kind with twelve rows. Each leg has its own segment lengths and socket on
  the body. The build step emits the rows in a loop. Adding a second crab must add rows and no new code.
- **Two eyes on stalks** come from a single mirrored definition. Each pupil looks at the same world-space target, a
  point that drifts slowly over time. Compute each eye's own world position correctly through the mirror, so the left
  eye and the right eye aim differently.
- **The material** is layered wet chitin: a red-orange base, a translucent subsurface layer, and a clear wet coat on
  top. Use a fine noise to vary the roughness. Keep the noise in the material, not in the distance field.

## Task 2: a reed bed in wind

Two hundred reeds stand in shallow water. Each reed is a thin, slightly tapered stalk with a seed head at the top.

- All 200 reeds are rows of one kind. Height, lean, stiffness and sway phase vary per reed. The build step places them
  in a loose, irregular cluster.
- Wind bends every reed over time. The bend grows toward the tip, a gust travels across the bed, and stiffer reeds bend
  less. The motion comes from each reed's own data and time; no reed has its own code.
- The look is not physically based: two flat inks on warm paper. Lit sides are paper. Shadowed sides take a dark ink,
  with a hard edge between the two. The water and the sky are plain paper with a faint second ink.

## Task 3: a heron's hop

A heron stands at the water's edge with a fish in view. Write the motion law and its checks, plus enough body geometry
to see the result: a trunk, two legs, an S-shaped neck, a head and a beak on the head's forward axis.

- The fish moves along a curve in world space. Its position is the gaze target.
- **The gaze lock comes first.** Each tick, the head keeps its eye aimed at the fish in world space, whatever the body
  is doing.
- **The neck has a limit.** The head may turn only so far from the trunk's heading. When the target passes beyond that
  limit, the head stops at the limit, and a constraint records the gaze as missed by the leftover angle. It must not
  report the gaze as held.
- **Then the body turns.** While the neck is at its limit, the heron makes a small hop: it leaves the ground, rotates
  its trunk toward the fish, and lands. Once the trunk has turned far enough, the neck comes back inside its limit and
  the gaze lock holds again.
- Add a second constraint that the head stays above the hips throughout. Schedule the laws in order: the body, then the
  neck and head, then the checks.
