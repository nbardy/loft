# Loft language card

A lean Lisp read by the Rust compiler straight into its typed graph, one form per node. A `defkind` is code; its
columns are a row. `!` heads run at build. Types are inferred except on columns, `pub` items and a law's or
check's first argument.

## Spaces and units

`(defspace World Body Leg)`; `(p3 Claw 0 0.2 0.9)` is metres. Types: `p3 v3 d3 n3`, `(xf A B)`, `f u bool rgb m
rad s`; `K` a row id, `[K]` a table. `[x y z]` takes space and type from context. Point + point, mixed spaces: E104.

## Kinds and rows

```clojure
(defkind CrabLeg :in Leg {:parent CrabBody :pose (xf Leg World) :femur m :foot (p3 World)}
  (trace [p] (capsule p [0 0 0] [femur 0 0] 0.07m)))
```

Columns are in scope. `(defkind Claw [serrated bool] …)` is a code variant; `[len m]`: E401.

## Build

```clojure
(defn! crab [b body]
  (for! [j (range 6) side [-1 1]]
    (emit! CrabLeg {:parent b :pose (* body (move [side 0 (* 0.25 j)])) :femur (+ 0.40m (* 0.05m j))
                    :foot (* body (p3 Body side 0 (* 0.3 j)))})))
(let! [at (place [0 0.4 0])] (crab (emit! CrabBody {:pose at}) at))
```

`emit!` returns a row id. `for!` unrolls; query `for` loops on GPU. `defmacro` returns forms, never rows.

## Queries

`(trace [p])`, `(surface [u v] :grid [64 32])`, `(visible [ro rd])`, `(material [p n])`, `light sky medium`;
`time` in scope. Bounds are derived. Zero-reaching divisor: E211. `(d f u)` of `min abs` Voronoi: E307.

```clojure
(defscan whorl [u 0 1 256] :on bake-lut
  :init (frame (spine u) (dir (d spine u)) [0 1 0])
  :step [prev] (transport prev (spine u) (dir (d spine u))))   ; (whorl u) -> C2 frame :o :n :b
```

## Maths

`($ b.com + b.vel * 0.28s)` is infix (`* /`, `+ -`, comparisons; left to right) over symbols, numbers or prefix
forms with optional `.f`; the reader rewrites it to prefix.

## Domain maps

```clojure
(let [sym (-> q mirror-x (move [-0.15 -0.72 -0.42]))
      c   (p3 sym 0.03 0.30 0.07)]
  (in sym (* (inv pose) (dir (- gaze (* pose (out sym c)))))))
```

A map is a point-bound value and a fresh space: `(:p sym)` is the mapped point, `(:cell sym)` the branch. Also `rot-x rot-z repeat-polar`.

## Materials

```clojure
(defn chitin [p] (+ (ggx [0.6 0.14 0.05] ($ 0.3 + 0.1 * (fbm2 p)) 0) (sss [0.9 0.3 0.1] 0.08) (coat 0.7 0.07)))
(deflobe ink [dark cut n v l] (if (< (dot n l) cut) dark [0.92 0.85 0.7]))
(defraw hash [p :v3] :-> f "return fract(sin(dot(p, vec3(12.9, 78.2, 37.7))) * 43758.5);")
```

Lobes sum bottom first: `ggx coat film sss emit`, `deflobe`. Untyped helpers are space-generic. Raw: no
derivative, `Unbounded`.

## Motion law

```clojure
(deflaw crab-step [leg :CrabLeg dt bodies] :writes [:foot]
  (let [b (bodies (:parent leg)) cp ($ b.com + b.vel * 0.28s)]
    (when (> (dist (:foot leg) cp) 0.5m) (write leg :foot [(:x cp) 0 (:z cp)]))))
```

Runs per row of its first argument's kind; a kind's laws have disjoint `:writes`.

## Constraint

```clojure
(defcheck upright [b :CrabBody] :-> m
  (let [margin ($ (* (:pose b) (:head b)).y - 0.15m)]
    (if (neg? margin) (missed (- margin)) (held margin))))
   ; or (unknown :tag)
```

## Schedule

```clojure
(emit! Sun {:dir (dir [-3 5 2]) :color [3 3 2.5]}) (emit! Sky {:zenith [0.4 0.5 0.6]})
(defschedule Main (crabs) (run-laws body-law crab-step) (check upright) (orbit [0 0.4 0] 6m 40deg)
  (draw Vis64 Ground CrabLeg) (shade Vis64 Sun Sky) (tonemap-aces Sky))
```

## Parse rules

1. Forms are `()` lists, `[]` vectors, `{}` maps; no precedence outside `$`.
2. `def…` heads declare (`defn!` a build fn), `…!` heads run at build, others call.
3. In argument vectors a keyword after a symbol is a type.
4. A row is a `{}` of exactly its columns.
5. A number touching letters is a unit or a lex error.
6. Capitalised symbols are spaces, kinds, tags.
7. `($ …)` is the sole infix form.
8. `a.f` only inside `$`; elsewhere `(:f a)`.
9. `write` only in a `deflaw`, on its `:writes`.
10. A `defraw` string is opaque WGSL.
