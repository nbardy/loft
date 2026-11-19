# Task 1

```clojure
(defspace World Body Leg Shell Claw Eyes)

(defn spine [t]
  (let [a ($ t * 18.0)
        r ($ 0.012m * (exp ($ 0.11 * a)))]
    (p3 Shell (* r (cos a)) ($ 0.006m * a) (* r (sin a)))))

(defscan whorl [u 0 1 256] :on bake-lut
  :init (frame (spine u) (dir (d spine u)) [0 1 0])
  :step [prev] (transport prev (spine u) (dir (d spine u))))

(defn shell-radius [u]
  ($ 0.010m * (exp ($ 0.11 * u * 18.0)) * (+ 1.0 (* 0.10 (sin ($ u * 400.0))))))

(defn hash1 [x y z] (hash [x y z]))

(defn chitin [p]
  (+ (ggx [0.85 0.28 0.10] ($ 0.22 + 0.18 * (fbm2 p)) 0)
     (sss [0.9 0.3 0.1] 0.08)
     (coat 0.7 0.07)))

(defn sand [p] (+ (ggx [0.45 0.38 0.28] 0.35 0) (coat 0.4 0.12)))

(defkind ShellK :in Shell {:pose (xf Shell World)}
  (surface [u v] :grid [64 32]
    (let [f (whorl u)
          ang ($ v * 6.2832rad)
          r (shell-radius u)]
      (* pose (+ (:o f) (* (* r (cos ang)) (:n f)) (* (* r (sin ang)) (:b f))))))
  (material [p n] (chitin p)))

(defkind CrabBody :in Body {:pose (xf Body World) :head (p3 Body)}
  (trace [p] (capsule p [-0.12m 0 0] [0.12m 0 0] 0.10m))
  (material [p n] (chitin p)))

(defkind CrabLeg :in Leg {:parent CrabBody :pose (xf Leg World) :femur m :tibia m :foot (p3 World)}
  (trace [p]
    (min (capsule p [0 0 0] [femur 0 0] 0.012m)
         (capsule p [femur 0 0] [femur (* -1 tibia) 0] 0.009m)))
  (material [p n] (chitin p)))

(defkind Claw :in Claw {:pose (xf Claw World) :open rad :scale f}
  (trace [p]
    (let [q (* (inv pose) p)
          s (/ q scale)
          palm (capsule s [0 0 0] [0.02m 0 0] 0.035m)
          fixed (capsule s [0.03m 0 0] [0.11m 0.01m 0] 0.012m)
          h (-> s (move [0.03m 0 0]) (rot-z (* -1 open)))
          hp (:p h)
          moving (+ (capsule hp [0 0 0] [0.08m 0 0] 0.011m)
                    ($ 0.003m * (sin ($ (:x hp) * 900.0))))]
      (* scale (min palm fixed moving))))
  (material [p n] (chitin p)))

(defn target [t]
  (p3 World ($ 0.4m * (sin ($ t * 0.3))) 0.15m ($ 0.6m + 0.2m * (cos ($ t * 0.21)))))

(defkind EyesK :in Eyes {:pose (xf Eyes World)}
  (trace [p]
    (let [sym (-> (* (inv pose) p) mirror-x (move [-0.03m 0.0m 0.0m]))
          c (p3 sym 0 0.04m 0)
          stalk (capsule (:p sym) [0 0 0] [0 0.04m 0] 0.005m)
          ball (capsule (:p sym) [0 0.04m 0] [0 0.04m 0] 0.012m)
          g (in sym (* (inv pose) (dir (- (target time) (* pose (out sym c))))))
          pupil (capsule (:p sym) [0 0.04m 0] (+ [0 0.04m 0] (* 0.011m g)) 0.006m)]
      (min stalk ball pupil)))
  (material [p n] (+ (ggx [0.05 0.05 0.05] 0.2 0) (coat 0.8 0.05))))

(defkind Ground :in World {}
  (trace [p] (:y p))
  (material [p n] (sand p)))

(defn! crab [b body]
  (for! [j (range 6) side [-1 1]]
    (emit! CrabLeg {:parent b
                    :pose (* body (move [(* side 0.08m) 0 (* 0.025m j)]))
                    :femur (+ 0.040m (* 0.005m j))
                    :tibia (+ 0.060m (* 0.004m j))
                    :foot (* body (p3 Body (* side 0.2m) 0 (* 0.03m j)))})))

(let! [at (place [0 0.05m 0])
       b (emit! CrabBody {:pose at :head (p3 Body 0.12m 0.04m 0)})]
  (crab b at)
  (emit! ShellK {:pose (* at (move [-0.05m 0.1m 0]))})
  (emit! Claw {:pose (* at (move [0.14m 0.02m 0.10m])) :open 0.5rad :scale 1.0})
  (emit! Claw {:pose (* at (move [0.14m 0.02m -0.10m])) :open 0.2rad :scale 0.6})
  (emit! EyesK {:pose (* at (move [0.12m 0.08m 0]))}))

(emit! Ground {})
(emit! Sun {:dir (dir [-3 5 2]) :color [3 3 2.5]})
(emit! Sky {:zenith [0.4 0.5 0.6]})

(defschedule Main (run-laws)
  (orbit [0 0.05m 0] 0.8m 40deg)
  (draw Vis64 Ground CrabBody CrabLeg Claw EyesK ShellK)
  (shade Vis64 Sun Sky)
  (tonemap-aces Sky))
```

# Task 2

```clojure
(defspace World Reed)

(deflobe ink [dark cut n v l]
  (if (< (dot n l) cut) dark [0.92 0.85 0.7]))

(defn paper [p n] (ink [0.12 0.10 0.30] 0.0))
(defn water-mat [p n] (+ (ink [0.55 0.58 0.62] -2.0)))
(defn sky-ink [p n] (ink [0.75 0.78 0.82] -2.0))

(defn gust [x t] (+ 0.5 (* 0.5 (sin ($ x * 1.3 - t * 1.6)))))

(defkind Reed :in Reed {:pose (xf Reed World) :height m :lean rad :stiff f :phase f}
  (trace [p]
    (let [q (* (inv pose) p)
          y (:y q)
          k ($ y / height)
          w ($ (gust (:x q) time) * (+ 0.6 (* 0.4 (sin ($ time * 1.1 + phase)))) * 0.25m / stiff)
          bend ($ (+ w (* height (sin lean))) * k * k)
          qb (p3 Reed (- (:x q) bend) y (:z q))
          stalk (capsule qb [0 0 0] [0 (* 0.92 height) 0] ($ 0.006m * (- 1.0 (* 0.5 k))))
          head (capsule qb [0 (* 0.92 height) 0] [0 height 0] 0.014m)]
      (min stalk head)))
  (material [p n] (paper p n)))

(defkind Water :in World {}
  (trace [p] (:y p))
  (material [p n] (water-mat p n)))

(defn! reeds []
  (for! [j (range 200)]
    (let! [a (* 6.2832 (hash [(* 1.0 j) 1.0 2.0]))
           r (* 1.5m (sqrt (hash [(* 1.0 j) 3.0 4.0])))
           h (+ 0.9m (* 0.6m (hash [(* 1.0 j) 5.0 6.0])))]
      (emit! Reed {:pose (place [(* r (cos a)) 0 (* r (sin a))])
                   :height h
                   :lean (* 0.12rad (- (hash [(* 1.0 j) 7.0 8.0]) 0.5))
                   :stiff (+ 0.6 (* 1.2 (hash [(* 1.0 j) 9.0 1.0])))
                   :phase (* 6.2832 (hash [(* 1.0 j) 2.0 9.0]))}))))

(defraw hash [p :v3] :-> f "return fract(sin(dot(p, vec3(12.9, 78.2, 37.7))) * 43758.5);")

(reeds)
(emit! Water {})
(emit! Sun {:dir (dir [-3 5 2]) :color [1 1 1]})
(emit! Sky {:zenith [0.92 0.88 0.8]})

(defschedule Main (run-laws)
  (orbit [0 0.6m 0] 4m 15deg)
  (draw Vis64 Water Reed)
  (shade Vis64 Sun Sky)
  (tonemap-aces Sky))
```

# Task 3

```clojure
(defspace World Trunk Head)

(defn wrap [a] (- (mod (+ a 3.1416rad) 6.2832rad) 3.1416rad))
(defn bearing [from to] (atan2 (- (:x to) (:x from)) (- (:z to) (:z from))))
(defn clampa [a lim] (max (* -1 lim) (min lim a)))

(defkind Trunk :in Trunk {:pose (xf Trunk World) :yaw rad :y m :vy f :air bool}
  (trace [p]
    (let [q (* (inv pose) p)
          body (capsule q [0 0.55m 0.12m] [0 0.60m -0.12m] 0.07m)
          legl (capsule q [0.04m 0.5m 0] [0.04m 0 0] 0.012m)
          legr (capsule q [-0.04m 0.5m 0] [-0.04m 0 0] 0.012m)]
      (min body legl legr)))
  (material [p n] (+ (ggx [0.5 0.55 0.6] 0.5 0))))

(defkind Neck :in Trunk {:pose (xf Trunk World)}
  (trace [p]
    (let [q (* (inv pose) p)]
      (min (capsule q [0 0.62m 0.12m] [0 0.80m 0.20m] 0.02m)
           (capsule q [0 0.80m 0.20m] [0 0.95m 0.12m] 0.02m)
           (capsule q [0 0.95m 0.12m] [0 1.05m 0.16m] 0.02m))))
  (material [p n] (+ (ggx [0.5 0.55 0.6] 0.5 0))))

(defkind HeronHead :in Head
  {:pose (xf Head World) :rel rad :over rad :limit rad :hips (p3 World)}
  (trace [p]
    (let [q (* (inv pose) p)
          skull (capsule q [0 0 0] [0 0 0.03m] 0.03m)
          beak (capsule q [0 0 0.03m] [0 0 0.20m] 0.008m)]
      (min skull beak)))
  (material [p n] (+ (ggx [0.8 0.7 0.2] 0.4 0))))

(defkind Fish :in World {:pos (p3 World) :s f}
  (trace [p] (capsule p (:pos (this)) (+ (:pos (this)) [0.06m 0 0]) 0.02m))
  (material [p n] (+ (ggx [0.7 0.7 0.8] 0.2 0) (coat 0.8 0.05))))

(deflaw fish-move [f :Fish dt] :writes [:pos :s]
  (let [s (+ (:s f) (* 0.12 dt))
        a ($ s * 6.2832)]
    (write f :s s)
    (write f :pos [(* 1.2m (cos a)) 0.05m (+ 1.0m (* 0.8m (sin a)))])))

(deflaw hop [t :Trunk dt fishes heads] :writes [:yaw :y :vy :air]
  (let [h (heads 0)
        fish (:pos (fishes 0))
        want (bearing [0 0 0] fish)
        err (wrap (- want (:yaw t)))]
    (cond
      (and (not (:air t)) (> (:over h) 0.0rad))
        (do (write t :vy 1.8) (write t :air true))
      (:air t)
        (let [vy (- (:vy t) (* 9.8 dt))
              y (+ (:y t) (* vy dt))]
          (write t :yaw (+ (:yaw t) (clampa err (* 6.0 dt))))
          (if (< y 0.0m)
            (do (write t :y 0.0m) (write t :vy 0.0) (write t :air false))
            (do (write t :y y) (write t :vy vy))))
      true nil)))

(deflaw gaze [h :HeronHead dt trunks fishes] :writes [:rel :over :pose :hips]
  (let [t (trunks 0)
        top (* (:pose t) (p3 Trunk 0 1.05m 0.16m))
        fish (:pos (fishes 0))
        want (wrap (- (bearing top fish) (:yaw t)))
        rel (clampa want (:limit h))
        over (max 0.0rad (- (abs want) (:limit h)))]
    (write h :rel rel)
    (write h :over over)
    (write h :hips (* (:pose t) (p3 Trunk 0 0.55m 0)))
    (write h :pose (* (:pose t) (move [0 1.05m 0.16m]) (rot-y rel)))))

(defcheck gaze-held [h :HeronHead] :-> rad
  (if (> (:over h) 0.0rad)
    (missed (:over h))
    (held (- (:limit h) (abs (:rel h))))))

(defcheck head-above-hips [h :HeronHead] :-> m
  (let [margin ($ (:y (* (:pose h) (p3 Head 0 0 0))) - (:y (:hips h)))]
    (if (neg? margin) (missed (- margin)) (held margin))))

(let! [at (place [0 0 0])]
  (emit! Trunk {:pose at :yaw 0.0rad :y 0.0m :vy 0.0 :air false})
  (emit! Neck {:pose at})
  (emit! HeronHead {:pose (* at (move [0 1.05m 0.16m])) :rel 0.0rad :over 0.0rad
                    :limit 1.0rad :hips (p3 World 0 0.55m 0)})
  (emit! Fish {:pos (p3 World 1.2m 0.05m 1.0m) :s 0.0}))

(emit! Sun {:dir (dir [-3 5 2]) :color [3 3 2.5]})
(emit! Sky {:zenith [0.4 0.5 0.6]})

(defschedule Main
  (run-laws fish-move hop gaze)
  (check gaze-held head-above-hips)
  (orbit [0 0.6m 0.5m] 3m 30deg)
  (draw Vis64 Trunk Neck HeronHead Fish)
  (shade Vis64 Sun Sky)
  (tonemap-aces Sky))
```
