# Art research 1: a 3D art tool for agents, from first principles

A handoff prompt, written 2026-11-11. It gives the goals and the reasons for them. It deliberately does not give a
design: a previous attempt exists, and you are free to keep none of it.

## The situation

We make a real-time game seen from a strategy camera: a sea, ships, weather, and large creatures that attack. The look
is stylised, not photoreal. Everything moves, and many things are on screen at once.

The artists are AI agents. They write the art. Nobody drags vertices in an editor. Players run the result in a browser
or a desktop client through WebGPU, often on integrated graphics.

## Why this needs rethinking

Every existing route fails an agent somewhere.

- Hand-written triangle code is long, and its edit-to-picture loop was 18 seconds in our game client.
- Modelling programs are built for hands and eyes.
- Distance-function shaders say a lot in a few lines, and were several times too slow for us.
- Baked assets are fast and opaque. An agent cannot reason about a file of vertices, or change one idea in it.

An agent is good at formulas, structure and arithmetic. It is poor at judging a picture. A tool for agents should
lean on the first and not depend on the second.

## What we want

In the owner's words (spelling tidied):

> instant compiling, instant rendering 3D objects that can animate, AND they can be expressed in the minimal amount of
> code (or data files) ... and downstream performance in user clients is strong

> not just a library for 3D art, but a new manner and method of drawing 3D art that is both more expressive and more
> powerful

> merging engines to get the best of both worlds, or leaving it behind and combining a new set of mathematics and
> algorithms

> the best of all different analytical, baked, marched, and traced effects in terms of details, techniques
> available, and devex, and rendering speed

> AI devex needs expression AND it needs to be in a format that the AI can use well, and we want it to be able to
> have very fast iterate loops

## What good looks like

- **Loop.** From an edit to a picture in about a second, on a laptop.
- **Size.** A creature fits on a page. Its named numbers are the things an art director would ask to change.
- **Client.** One object costs under a millisecond, with dozens to hundreds on screen. Work on the CPU counts too.
- **Checkable.** The agent can ask yes-or-no questions of its own work and get numbers back: does the attack land
  inside its warning, is anything thinner than a pixel, is the budget met.
- **Range.** Bodies, limbs, shells, ships, cloth, rope, sea, foam, clouds, lightning, and attacks that unfold in
  time. Outlines, and several styles from one description.
- **Comparable.** Any redesign can be rendered beside the last one on the same scenes, with the same measures.

## The whole kit, as needs

A language is not enough. Whatever you design has to come with a way to do each of these, and the loop has to stay
fast through all of them.

- See a thing, from several views, still and moving.
- Compare two versions of a thing, or one thing in several styles, side by side.
- Check a thing by arithmetic.
- Measure what a thing costs, per object, on the GPU and on the CPU.
- Hand a thing to the game client, and to tools that only read ordinary mesh files.
- Test the tool itself, so a change that breaks a picture is caught without a person looking.

## Questions to answer before you design

- What is a shape, when the author is a model? A surface, a solid, a function, a coordinate system, a program?
- What are the fewest kinds of thing that cover the range above, and what are the operators between them?
- What flows along the edges of the graph? The owner's criticism of an earlier design was that it passed only two
  kinds of value. He asked for any type: "fully flexible scripts as graphs".
- Where does the description of a thing stop and the way of drawing it start? Can one description be drawn several
  ways, and who chooses?
- What is computed once, and what every frame? Does the author decide, or the tool?
- Is time special, or one more dimension?
- How do things attach to each other: a rider on a creature, a ship on a wave, a wave bent by a whirlpool?
- Code or data? Which language, given that the author is a model and the loop must be instant? What runs in the
  client?
- How does style stay separate from shape?
- How does an agent know it is right without seeing?

## Seeds

Ideas the owner responded to. Use them, bend them or drop them.

- An attack as one object in space and time, so that its warning and its strike cannot disagree.
- A scene graph whose links may be curved: a thing lives in the coordinates of whatever carries it.
- A thing described as a coordinate system that other things can be written in.
- An interplay of formula, bake and sculpt, in steps and in either direction.
- The typed graphs of node editors, written as code.

## Freedom

There is a previous attempt (see Pointers). Treat it as one data point. Form your own outline first. Then read it,
and keep, change or replace any part. Nothing depends on it and no compatibility is owed.

If you think the goals above are wrongly posed, say so and pose them better.

## Hard constraints

- The author is an agent writing text.
- The result runs in WebGPU clients, including integrated graphics.
- Art is not compiled together with the game. That is what made the loop 18 seconds.
- Costs are measured on a real GPU, not estimated.

## What to deliver

1. A short design: the kinds of thing, the operators, what is computed when, and how it reaches the client.
2. A working slice in two styles: one creature, one ship on a moving sea, one weather effect, one attack. With
   pictures that you looked at.
3. Numbers: loop time, description size, and GPU and CPU time per object.
4. The same scenes in the previous attempt, with the same measures, side by side.
5. What you do not know, and what result would change your mind.

## How to work

- Measure before you optimise. We optimised the wrong thing more than once.
- Prefer a few ideas made exact to many ideas patched together.
- Write down what would kill an idea before you build it, and report it when it happens.
- Have a reader with fresh context recompute anything the design rests on.
- Use cheaper models for searching and bulk work, and keep the judgement yourself.

## Evidence from the previous attempt

Optional. Facts measured on 2026-11-11 on an M-series laptop at 1280x800, each of which you can measure again. They
are not doctrine. Skip this section if you would rather form your own view first.

- With art compiled as part of the game client, an edit took 18 seconds to reach the screen. In the previous
  attempt, from the command to a picture on disk took 0.45 seconds for a serpent and 1.4 seconds for a ship.
- A serpent's body was 62 named numbers.
- A whole serpent, drawn as triangles computed from formulas, cost about 0.15 ms of GPU time. A ship cost about
  0.3 ms, in 45 separate draws; its cost followed the number of draws, not triangles.
- A branching coral drawn by marching a distance function cost 12.6 ms. With distances cached it cost 4.5 ms. Listed
  as 3,069 simple shapes and intersected exactly it cost 1.4 ms.
- Cutting triangle counts by 2 to 2.7 times did not change frame time.
- In a frame with one creature and one ship, the per-frame pose work on the CPU was 5.4 ms, more than all drawing
  together.
- A cloud of 9 to 10 ms was the slowest single object left.

## Pointers

- The previous attempt: `~/git/loft` (`README.md`, `DESIGN.md`, `examples/`).
- The game: `~/git/room-runners-arena-lib`. Its client is Rust, under `keel/`. The older hand-written boss art is
  under `keel/games/art/`.
- The design note that led to the previous attempt: branch `art/boss-fable-pass` of the game repository,
  `notes/design/2026-11-10_creature_pipeline_options.md`.
- The companion prompt on the mathematics of drawing: `02_render_math_and_algorithms.md` in this folder.
