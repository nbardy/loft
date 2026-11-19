```clojure
; Task 1: hermit crab
(defspace World Body Leg Eye Claw Shell)

(defn spine [u]
  (let [r (* 0.06m (- 1 (* 0.5 u)))
        th (* 18.85 u)]
    (p3 World (* r (cos th)) (* r (sin th)) (* 0.05m u))))

(defn shell-r [u] (* 0.008m (+ 1 (* 2 u))))
(defn rib [u] (+ 1 (* 0.15 (sin (* 88 u)))))

(defscan whorl [u 0 1 256] :on bake-lut
  :init (frame (spine u) (dir (d spine u)) [0 1 0])
  :step [prev] (transport prev (spine u) (dir (d spine u))))

(defn chitin [p]
  (+ (ggx [0.75 0.22 0.06] ($ 0.25 + 0.08 * (fbm2 (* 60 p))) 0)
     (sss [0.9 0.35 0.15] 0.1)
     (coat 0.8 0.04)))

(defn sand-mat [p] (+ (ggx [0.3 0.27 0.22] 0.35 0) (coat 0.9 0.03)))

(defn claw-sdf [p open size]
  (let [palm (sphere p [0 0 0] (* 0.05m size))
        fixed (capsule p [0.02m 0m 0m] [0.09m 0m 0.02m] (* 0.012m size))
        q (-> p (rot-z open))
        serr (* 0.002m (abs (sin (* 2600 (/ (:x q) 1m)))))
        moving (- (capsule q [0m 0m 0m] [0.10m 0m 0m] (* 0.010m size)) serr)]
    (smin palm (min fixed moving) 0.01m)))

(defkind CrabBody :in Body {:pose (xf Body World) :head (p3 Body)}
  (trace [p] (sphere p [0 0 0] 0.07m))
  (material [p n] (chitin p)))

(defkind CrabLeg :in Leg {:parent CrabBody :pose (xf Leg World) :socket (p3 Body) :femur m :tibia m}
  (trace [p] (min (capsule p [0 0 0] [femur 0 0] 0.035m)
                  (capsule p [femur 0 0] [(+ femur tibia) 0 0] 0.025m)))
  (material [p n] (chitin p)))

(defkind Claw :in Claw {:parent CrabBody :pose (xf Claw World) :open rad :size f}
  (trace [p] (claw-sdf p open size))
  (material [p n] (chitin p)))

(defkind Eye :in Eye {:parent CrabBody :pose (xf Eye World) :socket (p3 Body) :aim (dir [0 0 1])}
  (trace [p] (min (capsule p [0 0 0] [0 0 0.08m] 0.008m)
                  (sphere p (+ [0 0 0.09m] (* 0.017m aim)) 0.02m)))
  (material [p n] (chitin p)))

(defkind Shell :in Shell {:pose (xf Shell World)}
  (surface [u v] :grid [256 32]
    (let [f (whorl u)
          a (* 6.28318 v)
          r (* (shell-r u) (rib u))]
      (+ (:o f) (* r (cos a) (:n f)) (* r (sin a) (:b f)))))
  (material [p n] (ggx [0.85 0.8 0.7] 0.3 0)))

(defkind Sand :in World {}
  (trace [p] (:y p))
  (material [p n] (sand-mat p)))

(defn! crab [b body]
  (for! [j (range 6) side [-1 1]]
    (emit! CrabLeg {:parent b
                    :pose (* body (move [(* side 0.02m) 0m (* 0.04m j)]))
                    :socket (p3 Body (* side 0.06m) 0m (* 0.04m j))
                    :femur (+ 0.40m (* 0.05m j))
                    :tibia (+ 0.30m (* 0.04m j))})))

(defn! claws [b body]
  (emit! Claw {:parent b :pose (* body (move [0.2m 0.1m 0.1m])) :open 0.6rad :size 1.0})
  (emit! Claw {:parent b :pose (* body (move [-0.2m 0.1m 0.1m])) :open 0.3rad :size 0.5}))

(defn! eyes [b body]
  (for! [side [-1 1]]
    (emit! Eye {:parent b
                :pose (* body (move [(* side 0.12m) 0.2m 0.1m]))
                :socket (p3 Body (* side 0.12m) 0.2m 0.1m)
                :aim (dir [0 0 1])})))

(deflaw eye-aim [eye :Eye] :writes [:aim]
  (let [pose (:pose eye)
        sym (-> (:socket eye) mirror-x)
        c (p3 sym 0m 0m 0.07m)
        gaze (p3 World (* 0.2m (sin (* 0.1 time))) 0.1m 0.4m)]
    (write eye :aim (in sym (* (inv pose) (dir (- gaze (* pose (out sym c)))))))))

(defcheck upright [b :CrabBody] :-> m
  (let [top (* (:pose b) (:head b))
        margin ($ top.y - 0.15m)]
    (if (neg? margin) (missed (- margin)) (held margin))))

(let! [at (place [0 0.4 0])
       b (emit! CrabBody {:pose at :head (p3 Body 0m 0.1m 0.11m)})]
  (crab b at)
  (claws b at)
  (eyes b at)
  (emit! Shell {:pose at})
  (emit! Sand {}))

(emit! Sun {:dir (dir [-3 5 2]) :color [3 3 2.5]}) (emit! Sky {:zenith [0.4 0.5 0.6]})

(defschedule Main
  (run-laws eye-aim) (check upright) (orbit [0 0.2 0] 1.5m 30deg)
  (draw Vis64 Sand CrabBody CrabLeg Claw Eye Shell) (shade Vis64 Sun Sky) (tonemap-aces Sky))
```

```clojure
; Task 2: reed bed in wind
(defspace World Reed)

(defkind Reed :in Reed {:root (p3 World) :height m :lean rad :stiff f :phase f}
  (trace [p]
    (let [now (/ time 1s)
          g (+ 0.6 (* 0.4 (sin (- (* 1.3 now) (* 1.7 (/ (:x root) 1m))))))
          theta (+ lean (* (/ 0.5rad (+ 1 stiff)) g) (* 0.1rad (sin (+ phase (* 2.2 now)))))
          mid [(* 0.5 height (sin (* 0.5 theta))) (* 0.5 height (cos (* 0.5 theta))) 0m]
          tip [(* height (sin theta)) (* height (cos theta)) 0m]]
      (min (capsule p [0m 0m 0m] mid 0.007m)
           (capsule p mid tip 0.003m)
           (capsule p (- tip [0m 0.01m 0m]) (+ tip [0m 0.01m 0m]) 0.008m))))
  (material [p n] (ink [0.12 0.09 0.14] 0.05)))

(defkind Water :in World {}
  (trace [p] (:y p))
  (material [p n] (ink [0.86 0.82 0.72] -0.5)))

(defn! bed []
  (for! [k (range 200)]
    (emit! Reed {:root (p3 World (* 1.6m (sin (* 12.9 k))) 0m (* 1.2m (cos (* 7.3 k))))
                 :height (+ 0.9m (* 0.5m (fract (* 3.7 k))))
                 :lean (* 0.08rad (sin (* 5.1 k)))
                 :stiff (+ 0.2 (* 0.8 (fract (* 2.3 k))))
                 :phase (* 6.28 (fract (* 0.61 k)))})))

(bed)
(emit! Water {})
(emit! Sun {:dir (dir [-1 2 1]) :color [1 1 1]}) (emit! Sky {:zenith [0.98 0.94 0.84]})

(defschedule Main
  (orbit [0 0.5m 0] 4m 15deg)
  (draw Vis64 Water Reed) (shade Vis64 Sun Sky) (tonemap-aces Sky))
```

```clojure
; Task 3: heron's hop
(defspace World Body)

(defn fish-pos [t]
  (p3 World (* 2.0m (cos (* 0.2 (/ t 1s)))) 0.05m (* 2.0m (sin (* 0.2 (/ t 1s))))))

(defn wrap-angle [a] (- (mod (+ a 3.14159rad) 6.28318rad) 3.14159rad))

(defn yaw-to [from to] (atan2 (- (:x to) (:x from)) (- (:z to) (:z from))))

(defn approach [cur goal step]
  (+ cur (clamp (wrap-angle (- goal cur)) (- 0rad step) step)))

(defkind Ground :in World {}
  (trace [p] (:y p))
  (material [p n] (ggx [0.35 0.32 0.27] 0.5 0)))

(defkind Fish :in World {}
  (trace [p] (sphere p (fish-pos time) 0.06m))
  (material [p n] (ink [0.9 0.5 0.1] -0.5)))

(defkind Heron :in Body
  {:pose (xf Body World) :pos (p3 World) :yaw rad :lift m :hop s
   :turn rad :limit rad :limited bool :gap rad}
  (trace [p]
    (let [q (-> p (move [0.08m -1.0m -0.2m]) (rot-y (- 0rad turn)))]
      (min (sphere p [0 0.3m 0] 0.13m)
           (capsule p [-0.08m 0.2m 0m] [-0.08m -0.9m 0.05m] 0.02m)
           (capsule p [0.08m 0.2m 0m] [0.08m -0.9m 0.05m] 0.02m)
           (capsule p [0m 0.4m 0.05m] [0.1m 0.8m 0.12m] 0.03m)
           (capsule p [0.1m 0.8m 0.12m] [-0.08m 1.0m 0.2m] 0.025m)
           (sphere q [0m 0m 0m] 0.07m)
           (capsule q [0m 0m 0.02m] [0m 0m 0.22m] 0.018m))))
  (material [p n] (ggx [0.45 0.48 0.5] 0.4 0)))

(deflaw trunk-hop [h :Heron dt] :writes [:pose :yaw :lift :hop]
  (let [air (if (>= (:hop h) 0s) (+ (:hop h) dt) (if (:limited h) 0s -1s))
        hop (if (< air 0.4s) air -1s)
        lift (if (>= hop 0s) (* 0.12m (sin (* 3.14159 (/ hop 0.4s)))) 0m)
        yaw (if (>= hop 0s)
              (approach (:yaw h) (yaw-to (:pos h) (fish-pos time)) (* 1.5rad (/ dt 1s)))
              (:yaw h))]
    (write h :hop hop)
    (write h :lift lift)
    (write h :yaw yaw)
    (write h :pose (* (move [(:x (:pos h)) (+ 0.9m lift) (:z (:pos h))]) (rot-y yaw)))))

(deflaw head-gaze [h :Heron] :writes [:turn :limited :gap]
  (let [rel (wrap-angle (- (yaw-to (:pos h) (fish-pos time)) (:yaw h)))
        turn (clamp rel (- 0rad (:limit h)) (:limit h))]
    (write h :turn turn)
    (write h :limited (> (abs rel) (:limit h)))
    (write h :gap (- rel turn))))

(defcheck gaze-held [h :Heron] :-> rad
  (let [gap (abs (:gap h))]
    (if (> gap 0.001rad) (missed gap) (held (- 0.001rad gap)))))

(defcheck head-above-hips [h :Heron] :-> m
  (let [head (* (:pose h) (p3 Body -0.08m 1.0m 0.2m))
        hips (* (:pose h) (p3 Body 0m 0.3m 0m))
        margin ($ head.y - hips.y)]
    (if (neg? margin) (missed (- margin)) (held margin))))

(emit! Ground {}) (emit! Fish {})
(emit! Heron {:pose (move [0m 0.9m 0m]) :pos (p3 World 0m 0m 2m)
              :yaw 0rad :lift 0m :hop -1s :turn 0rad :limit 1.2rad :limited false :gap 0rad})
(emit! Sun {:dir (dir [-3 5 2]) :color [3 3 2.5]}) (emit! Sky {:zenith [0.4 0.5 0.6]})

(defschedule Main
  (run-laws trunk-hop head-gaze)
  (check gaze-held) (check head-above-hips)
  (orbit [0 0.5m 0] 4m 30deg)
  (draw Vis64 Ground Heron Fish) (shade Vis64 Sun Sky) (tonemap-aces Sky))
```
