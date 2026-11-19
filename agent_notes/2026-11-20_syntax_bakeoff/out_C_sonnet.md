# Task 1

```hlsl
struct World : ISpace {};
struct Body : ISpace {};
struct Leg : ISpace {};
struct Claw : ISpace {};
struct Shell : ISpace {};
struct Face : ISpace {};

// ---- shell: scanned frames along a spiral spine ----
Pt<Shell> spine(float u)
{
    float a = 14.0 * u;                       // radians of coil
    float r = 0.02 * exp(0.9 * u);            // metres, grows with the spiral
    return Pt<Shell>(r * cos(a), 0.11 * u, r * sin(a));
}

[Scan(steps = 256, range = (0.0, 1.0), BakeLut)]
struct Whorl : IScan<Frame<Shell>>
{
    Frame<Shell> init(float u)
    {
        return frame_from_tangent(spine(u), normalize(diff(spine, u)(u)), Dir<Shell>(0, 1, 0));
    }
    Frame<Shell> step(Frame<Shell> prev, float u)
    {
        return transport_frame(prev, spine(u), normalize(diff(spine, u)(u)));
    }
};

[Kind]
struct ShellRow : ISurface<Shell>, IMaterial<Shell>
{
    RowId<CrabBody> parent;
    Xform<Shell, World> pose;
    float3 albedo;

    [Grid(256, 48)]
    Pt<Shell> surface(float u, float v)
    {
        Frame<Shell> f = sample_c2<Whorl>(u);
        float a = v * 6.2831853;
        float rib = 1.0 + 0.08 * sin(40.0 * u * 6.2831853) ;
        float radius = (0.012 + 0.07 * u) * rib;     // tube grows with the spiral
        return f.origin + (f.normal * cos(a) + f.binormal * sin(a)) * radius;
    }

    Bsdf material(Pt<Shell> p, Norm<Shell> n)
    {
        return ggx(albedo, 0.45, 0.0) + clearcoat(0.3, 0.2);
    }
};

// ---- claw: one definition, differences are data ----

[Kind]
struct ClawRow : ITrace<Claw>, IMaterial<Claw>
{
    RowId<CrabBody> parent;
    Xform<Claw, World> pose;
    Meters size;          // palm radius
    Rad open;             // hinge opening angle, per instance
    float serration;      // tooth amplitude, metres

    Field<Claw> trace(Pt<Claw> p)
    {
        Field<Claw> palm = sd_round_box(p, Vec<Claw>(size, 0.7 * size, 0.6 * size), 0.2 * size);

        // fixed finger
        Field<Claw> fixed = sd_capsule(p, Pt<Claw>(size, 0, 0), Pt<Claw>(2.2 * size, -0.2 * size, 0), 0.22 * size);

        // moving finger, hinged at the palm
        Pt<Claw> h = Pt<Claw>(size, 0.4 * size, 0);
        Vec<Claw> q = p - h;
        float c = cos(open);
        float s = sin(open);
        Pt<Claw> pm = Pt<Claw>(c * q.x + s * q.y, -s * q.x + c * q.y, q.z);
        float teeth = serration * sin(60.0 * pm.x);
        Field<Claw> mover = sd_capsule(pm, Pt<Claw>(0, 0, 0), Pt<Claw>(1.8 * size, -0.2 * size, 0), 0.2 * size) + teeth;

        return smin_poly(smin_poly(palm, fixed, 0.1 * size), mover, 0.05 * size);
    }

    Bsdf material(Pt<Claw> p, Norm<Claw> n)
    {
        return chitin(p);
    }
};

// ---- legs ----
[Kind]
struct CrabLeg : ITrace<Leg>, IMaterial<Leg>
{
    RowId<CrabBody> parent;
    Xform<Leg, World> pose;
    Meters femur;
    Meters tibia;
    Pt<World> foot;

    Field<Leg> trace(Pt<Leg> p)
    {
        Field<Leg> a = sd_capsule(p, Pt<Leg>(0, 0, 0), Pt<Leg>(femur, 0, 0), 0.03m);
        Field<Leg> b = sd_capsule(p, Pt<Leg>(femur, 0, 0), Pt<Leg>(femur, -tibia, 0), 0.022m);
        return smin_poly(a, b, 0.02m);
    }

    Bsdf material(Pt<Leg> p, Norm<Leg> n) { return chitin(p); }
};

// ---- body ----
[Kind]
struct CrabBody : ITrace<Body>, IMaterial<Body>
{
    Xform<Body, World> pose;
    Pt<World> com;
    Vec<World> vel;

    Field<Body> trace(Pt<Body> p)
    {
        return sd_round_box(p, Vec<Body>(0.14m, 0.05m, 0.18m), 0.04m);
    }

    Bsdf material(Pt<Body> p, Norm<Body> n) { return chitin(p); }
};

// ---- eyes: one mirrored definition ----
[Kind]
struct EyeRow : ITrace<Face>, IMaterial<Face>
{
    RowId<CrabBody> parent;
    Xform<Face, World> pose;
    Pt<World> gaze_world;     // written by eye_law

    Field<Face> trace(Pt<Face> q, Sec time)
    {
        let sym = map(q).mirror_x().translate(-Vec<Face>(0.05, 0.0, 0.0));
        let eye_c = sym.pt(0.0, 0.06, 0.0);
        Field<Face> stalk = sd_capsule(sym.p, Pt<sym.Space>(0, 0, 0), eye_c, 0.012m);
        Field<Face> globe = sd_sphere(sym.p - eye_c, 0.028m);

        // gaze target drifts over time, in world space
        Pt<World> target = Pt<World>(0.5 * sin(0.3 * time / 1.0s), 0.1, 0.8 + 0.2 * cos(0.2 * time / 1.0s));
        Pt<World> eye_w = pose * sym.to_parent(eye_c);
        let look = sym.from_parent(inverse(pose) * normalize(target - eye_w));
        Pt<sym.Space> pupil_c = eye_c + Vec<sym.Space>(look.x, look.y, look.z) * 0.024;
        Field<Face> pupil = sd_sphere(sym.p - pupil_c, 0.011m);

        return hit_min(smin_poly(stalk, globe, 0.01m), pupil);
    }

    Bsdf material(Pt<Face> p, Norm<Face> n)
    {
        return ggx(float3(0.02, 0.02, 0.02), 0.1, 0.0) + clearcoat(1.0, 0.03);
    }
};

// ---- material: layered wet chitin, noise only here ----
Bsdf chitin(Pt<Body> p)
{
    float r = 0.35 + 0.2 * fbm2(float2(p.x, p.z) * 90.0);
    return ggx(float3(0.75, 0.22, 0.08), r, 0.0) + sss(float3(0.9, 0.3, 0.1), 0.08) + clearcoat(0.7, 0.07);
}

// ---- sand, light, sky ----
[Kind]
struct Ground : ITrace<World>, IMaterial<World>
{
    Field<World> trace(Pt<World> p) { return p.y + 0.004 * fbm2(float2(p.x, p.z) * 20.0); }
    Bsdf material(Pt<World> p, Norm<World> n)
    {
        return ggx(float3(0.32, 0.26, 0.18), 0.2, 0.0) + clearcoat(0.5, 0.05);
    }
};

[Kind]
struct SunLight : ILight
{
    Dir<World> dir;
    float3 color;
};

[Kind]
struct SkyRow : ISky
{
    float3 zenith;
    float3 horizon;
};

// ---- laws ----
[Law(Writes = CrabBody)]
void body_law(inout CrabBody b, Sec dt)
{
    b.vel = Vec<World>(0.02, 0.0, 0.0);
    b.com = b.com + b.vel * dt;
}

[Law(Writes = CrabLeg)]
void crab_step(inout CrabLeg leg, Sec dt, Table<CrabBody> bodies)
{
    CrabBody b = bodies[leg.parent];
    Pt<World> cp = b.com + b.vel * 0.28s;
    if (distance(leg.foot, cp) > 0.5m) leg.foot = Pt<World>(cp.x, 0.0, cp.z);
}

// ---- rows the build emits ----
[Comptime]
void crab(RowId<CrabBody> b, Xform<Body, World> body)
{
    for (uint j = 0; j < 6; j++)
        for (float side = -1.0; side <= 1.0; side += 2.0)
            emit(CrabLeg{ .parent = b, .pose = body * leg_socket(j, side),
                          .femur = 0.10m + 0.012m * j, .tibia = 0.09m + 0.01m * j,
                          .foot = body * Pt<Body>(side * 0.25, 0.0, 0.05 * j - 0.12) });

    emit(ClawRow{ .parent = b, .pose = body * claw_socket(-1.0), .size = 0.05m, .open = 0.45rad, .serration = 0.004 });
    emit(ClawRow{ .parent = b, .pose = body * claw_socket(1.0),  .size = 0.025m, .open = 0.2rad, .serration = 0.0 });

    emit(EyeRow{ .parent = b, .pose = body * eye_socket(), .gaze_world = Pt<World>(0, 0, 0) });
    emit(ShellRow{ .parent = b, .pose = body * shell_socket(), .albedo = float3(0.85, 0.7, 0.55) });
}

[Comptime]
void crabs()
{
    Xform<Body, World> body = translate_xform<Body, World>(Vec<World>(0.0, 0.08, 0.0));
    RowId<CrabBody> b = emit(CrabBody{ .pose = body, .com = Pt<World>(0.0, 0.08, 0.0), .vel = Vec<World>(0, 0, 0) });
    crab(b, body);
    emit(Ground{});
    emit(SunLight{ .dir = normalize(Dir<World>(-0.4, -0.8, -0.3)), .color = float3(1.0, 0.95, 0.85) });
    emit(SkyRow{ .zenith = float3(0.35, 0.55, 0.85), .horizon = float3(0.8, 0.85, 0.9) });
}

[Constraint]
Verdict<Meters> crab_on_sand(CrabBody b)
{
    if (!is_finite(b.pose)) return Verdict::Unknown(NonFinitePose);
    Meters margin = b.com.y - 0.02m;
    if (margin < 0.0m) return Verdict::Missed(-margin);
    return Verdict::Held(margin);
}

technique Main
{
    pass Rows     { crabs(); }
    pass Motion   { run_laws(body_law, crab_step); }
    pass Check    { check(crab_on_sand); }
    pass Camera   { orbit(Pt<World>(0.0, 0.1, 0.0), 0.9m, 25deg); }
    pass Cull     { build_indirect<Ground, CrabLeg, ClawRow>(); }
    pass Vis      { render_bundle(Vis64) { draw_indirect<Ground, CrabLeg, ClawRow>(); } }
    pass Classify { tile_bin_kinds(Vis64, 8); }
    pass Shade    { dispatch_material_tiles(Vis64, sun_sky); }
    pass Present  { tonemap_aces(ColorHdr, sky_color); }
}
```

# Task 2

```hlsl
struct World : ISpace {};
struct Reed : ISpace {};

// wind bend angle at height fraction h along the stalk; gust travels along +x
Rad bend_angle(Pt<World> base, float h, Sec time, Rad lean, float stiffness, Rad phase)
{
    float gust = 0.5 + 0.5 * sin(1.2 * (base.x / 1.0m) - 1.5 * (time / 1.0s));
    float sway = sin(2.1 * (time / 1.0s) + phase / 1.0rad);
    return lean + (0.35rad * gust + 0.08rad * sway) * h * h / (0.5 + stiffness);
}

[Kind]
struct ReedRow : ITrace<Reed>, IMaterial<Reed>
{
    Xform<Reed, World> pose;
    Meters height;
    Rad lean;
    float stiffness;
    Rad phase;

    Field<Reed> trace(Pt<Reed> p, Sec time)
    {
        // bend: shear the point back by the arc displacement at its height
        float h = clamp(p.y / height, 0.0, 1.0);
        Rad a = bend_angle(pose * Pt<Reed>(0, 0, 0), h, time, lean, stiffness, phase);
        float c = cos(a);
        float s = sin(a);
        Pt<Reed> q = Pt<Reed>(c * p.x + s * p.y - s * 0.0, -s * p.x + c * p.y, p.z);
        Pt<Reed> tip = Pt<Reed>(0.0, height, 0.0);
        Meters r = 0.006m * (1.0 - 0.6 * h);
        Field<Reed> stalk = sd_capsule(q, Pt<Reed>(0, 0, 0), tip, r);
        Field<Reed> head = sd_capsule(q, tip, tip + Vec<Reed>(0, 0.09m, 0), 0.014m);
        return smin_poly(stalk, head, 0.01m);
    }

    Bsdf material(Pt<Reed> p, Norm<Reed> n)
    {
        return ink(float3(0.18, 0.2, 0.28), 0.15);
    }
};

[Lobe]
float4 ink(float3 dark, float cut, float3 n, float3 v, float3 l)
{
    return float4(dot(n, l) < cut ? dark : float3(0.92, 0.85, 0.7), 0.0);
}

[Kind]
struct Water : ITrace<World>, IMaterial<World>
{
    Field<World> trace(Pt<World> p) { return p.y - 0.0; }
    Bsdf material(Pt<World> p, Norm<World> n)
    {
        return ink(float3(0.92, 0.85, 0.7), 2.0) + ink(float3(0.6, 0.66, 0.7), 0.0 + 0.9 * fbm2(float2(p.x, p.z) * 4.0));
    }
};

[Kind]
struct PaperSky : ISky
{
    float3 paper;
    float3 faint;
};

[Kind]
struct Sun : ILight
{
    Dir<World> dir;
    float3 color;
};

float hash1(float n)
{
    __wgsl(n) { return fract(sin(n * 91.3458) * 47453.5453); }
}

[Comptime]
void reeds()
{
    for (uint i = 0; i < 200; i++)
    {
        float fi = float(i);
        float ang = 6.2831853 * hash1(fi * 1.7);
        float rad = 1.6 * sqrt(hash1(fi * 3.1)) * (0.6 + 0.4 * hash1(fi * 5.3));
        Pt<World> base = Pt<World>(rad * cos(ang), 0.0, rad * sin(ang));
        emit(ReedRow{
            .pose = translate_xform<Reed, World>(Vec<World>(base.x, 0.0, base.z)),
            .height = 0.9m + 0.7m * hash1(fi * 7.7),
            .lean = (hash1(fi * 11.3) - 0.5) * 0.3rad,
            .stiffness = 0.3 + 1.2 * hash1(fi * 13.9),
            .phase = 6.2831853rad * hash1(fi * 17.1) });
    }
    emit(Water{});
    emit(PaperSky{ .paper = float3(0.92, 0.85, 0.7), .faint = float3(0.75, 0.72, 0.62) });
    emit(Sun{ .dir = normalize(Dir<World>(-0.5, -0.7, -0.4)), .color = float3(1, 1, 1) });
}

technique Main
{
    pass Rows     { reeds(); }
    pass Camera   { orbit(Pt<World>(0.0, 0.7, 0.0), 4m, 15deg); }
    pass Cull     { build_indirect<Water, ReedRow>(); }
    pass Vis      { render_bundle(Vis64) { draw_indirect<Water, ReedRow>(); } }
    pass Classify { tile_bin_kinds(Vis64, 8); }
    pass Shade    { dispatch_material_tiles(Vis64, sun_sky); }
    pass Present  { tonemap_aces(ColorHdr, sky_color); }
}
```

# Task 3

```hlsl
struct World : ISpace {};
struct Trunk : ISpace {};
struct Head : ISpace {};
struct Limb : ISpace {};

static const Rad NECK_LIMIT = 1.1rad;
static const Rad TURN_DONE = 0.15rad;

[Kind]
struct Fish : ITrace<World>, IMaterial<World>
{
    Pt<World> pos;
    Sec clock;

    Field<World> trace(Pt<World> p) { return sd_sphere(p - pos, 0.05m); }
    Bsdf material(Pt<World> p, Norm<World> n) { return ggx(float3(0.7, 0.7, 0.75), 0.2, 0.8); }
};

[Kind]
struct HeronTrunk : ITrace<Trunk>, IMaterial<Trunk>
{
    Xform<Trunk, World> pose;
    Pt<World> pos;
    Rad heading;          // yaw about +y
    float hop_t;          // 0 = grounded, (0,1] = airborne
    bool hopping;

    Field<Trunk> trace(Pt<Trunk> p)
    {
        Field<Trunk> body = sd_capsule(p, Pt<Trunk>(0, 0, -0.12m), Pt<Trunk>(0, 0.05m, 0.12m), 0.09m);
        Field<Trunk> legl = sd_capsule(p, Pt<Trunk>(-0.04m, 0, 0), Pt<Trunk>(-0.04m, -0.55m, 0), 0.012m);
        Field<Trunk> legr = sd_capsule(p, Pt<Trunk>(0.04m, 0, 0), Pt<Trunk>(0.04m, -0.55m, 0), 0.012m);
        return smin_poly(body, hit_min(legl, legr), 0.02m);
    }
    Bsdf material(Pt<Trunk> p, Norm<Trunk> n) { return ggx(float3(0.45, 0.5, 0.58), 0.6, 0.0); }
};

[Kind]
struct HeronNeck : ITrace<Trunk>, IMaterial<Trunk>
{
    RowId<HeronTrunk> parent;
    Xform<Head, World> head_pose;
    Rad yaw;              // head yaw relative to trunk heading
    Rad lag;              // leftover angle beyond the limit, 0 when inside
    bool at_limit;

    // S-shaped neck from shoulder to head, in trunk space; head sits on the end
    Field<Trunk> trace(Pt<Trunk> p)
    {
        Pt<Trunk> s0 = Pt<Trunk>(0, 0.10m, 0.12m);
        Pt<Trunk> s1 = Pt<Trunk>(0, 0.30m, 0.20m);
        Pt<Trunk> s2 = Pt<Trunk>(0, 0.38m, 0.08m);
        Pt<Trunk> s3 = Pt<Trunk>(0, 0.55m, 0.16m);
        Field<Trunk> n = smin_poly(sd_capsule(p, s0, s1, 0.02m),
                         smin_poly(sd_capsule(p, s1, s2, 0.02m), sd_capsule(p, s2, s3, 0.018m), 0.02m), 0.02m);
        Field<Trunk> beak = sd_capsule(p, s3, s3 + Vec<Trunk>(0, 0, 0.16m), 0.01m);
        Field<Trunk> skull = sd_sphere(p - s3, 0.03m);
        return smin_poly(n, smin_poly(skull, beak, 0.01m), 0.01m);
    }
    Bsdf material(Pt<Trunk> p, Norm<Trunk> n) { return ggx(float3(0.8, 0.8, 0.82), 0.5, 0.0); }
};

Rad wrap_pi(Rad a)
{
    return a - 6.2831853rad * floor((a + 3.1415927rad) / 6.2831853rad);
}

Pt<World> fish_at(Sec t)
{
    float s = t / 1.0s;
    return Pt<World>(1.2 + 0.9 * sin(0.4 * s), 0.0, 1.0 + 1.6 * sin(0.4 * s + 1.0) * cos(0.25 * s));
}

[Law(Writes = Fish)]
void fish_law(inout Fish f, Sec dt)
{
    f.clock = f.clock + dt;
    f.pos = fish_at(f.clock);
}

// bearing of a world point from the trunk, relative to trunk heading
Rad bearing(HeronTrunk b, Pt<World> target)
{
    Vec<World> d = target - b.pos;
    return wrap_pi(atan2(d.x, d.z) * 1.0rad - b.heading);
}

[Law(Writes = HeronTrunk)]
void body_law(inout HeronTrunk b, Sec dt, Table<HeronNeck> necks, Table<Fish> fish)
{
    HeronNeck n = necks[RowId<HeronNeck>(0)];
    Pt<World> target = fish[RowId<Fish>(0)].pos;
    Rad rel = bearing(b, target);

    // start a hop when the neck is pinned at its limit
    if (!b.hopping && n.at_limit) { b.hopping = true; b.hop_t = 0.0; }

    if (b.hopping)
    {
        b.hop_t = b.hop_t + (dt / 0.5s);
        float h = 0.12 * sin(3.1415927 * clamp(b.hop_t, 0.0, 1.0));    // metres
        b.pos = Pt<World>(b.pos.x, h, b.pos.z);
        // rotate the trunk toward the fish while airborne, fraction of the remaining angle
        b.heading = b.heading + rel * clamp(4.0 * (dt / 1.0s), 0.0, 1.0);
        if (b.hop_t >= 1.0)
        {
            b.hopping = false;
            b.hop_t = 0.0;
            b.pos = Pt<World>(b.pos.x, 0.0, b.pos.z);
        }
    }
    b.pose = translate_xform<Trunk, World>(Vec<World>(b.pos.x, b.pos.y + 0.6, b.pos.z)) * rot_y_xform<Trunk, Trunk>(b.heading);
}

[Law(Writes = HeronNeck)]
void neck_law(inout HeronNeck n, Sec dt, Table<HeronTrunk> trunks, Table<Fish> fish)
{
    HeronTrunk b = trunks[n.parent];
    Rad want = bearing(b, fish[RowId<Fish>(0)].pos);     // gaze lock: aim at the fish in world space

    if (abs(want) > NECK_LIMIT)
    {
        n.yaw = want > 0.0rad ? NECK_LIMIT : -NECK_LIMIT;
        n.lag = abs(want) - NECK_LIMIT;
        n.at_limit = true;
    }
    else
    {
        n.yaw = want;
        n.lag = 0.0rad;
        n.at_limit = abs(want) > NECK_LIMIT - TURN_DONE ? n.at_limit : false;
    }
    n.head_pose = b.pose * rot_y_xform<Head, Trunk>(n.yaw);
}

[Constraint]
Verdict<Rad> gaze(HeronNeck n, Table<HeronTrunk> trunks, Table<Fish> fish)
{
    if (!is_finite(n.head_pose)) return Verdict::Unknown(NonFinitePose);
    if (n.lag > 0.0rad) return Verdict::Missed(n.lag);
    return Verdict::Held(NECK_LIMIT - abs(n.yaw));
}

[Constraint]
Verdict<Meters> head_above_hips(HeronNeck n, Table<HeronTrunk> trunks)
{
    HeronTrunk b = trunks[n.parent];
    if (!is_finite(n.head_pose)) return Verdict::Unknown(NonFinitePose);
    Meters margin = (n.head_pose * Pt<Head>(0, 0, 0)).y + 0.55m - (b.pose * Pt<Trunk>(0, 0, 0)).y;
    if (margin < 0.0m) return Verdict::Missed(-margin);
    return Verdict::Held(margin);
}

[Kind]
struct Shore : ITrace<World>, IMaterial<World>
{
    Field<World> trace(Pt<World> p) { return p.y; }
    Bsdf material(Pt<World> p, Norm<World> n) { return ggx(float3(0.3, 0.27, 0.2), 0.5, 0.0); }
};

[Kind]
struct Sun : ILight { Dir<World> dir; float3 color; };

[Kind]
struct Sky : ISky { float3 zenith; float3 horizon; };

[Comptime]
void scene()
{
    RowId<HeronTrunk> t = emit(HeronTrunk{
        .pose = translate_xform<Trunk, World>(Vec<World>(0, 0.6, 0)), .pos = Pt<World>(0, 0, 0),
        .heading = 0.0rad, .hop_t = 0.0, .hopping = false });
    emit(HeronNeck{ .parent = t, .head_pose = translate_xform<Head, World>(Vec<World>(0, 1.15, 0)),
                    .yaw = 0.0rad, .lag = 0.0rad, .at_limit = false });
    emit(Fish{ .pos = fish_at(0.0s), .clock = 0.0s });
    emit(Shore{});
    emit(Sun{ .dir = normalize(Dir<World>(-0.3, -0.8, -0.4)), .color = float3(1.0, 0.96, 0.9) });
    emit(Sky{ .zenith = float3(0.4, 0.6, 0.9), .horizon = float3(0.85, 0.88, 0.92) });
}

technique Main
{
    pass Rows     { scene(); }
    pass Motion   { run_laws(fish_law, body_law, neck_law); }
    pass Check    { check(gaze); check(head_above_hips); }
    pass Camera   { orbit(Pt<World>(0.0, 0.7, 0.0), 4m, 20deg); }
    pass Cull     { build_indirect<Shore, HeronTrunk, HeronNeck>(); }
    pass Vis      { render_bundle(Vis64) { draw_indirect<Shore, HeronTrunk, HeronNeck>(); } }
    pass Classify { tile_bin_kinds(Vis64, 8); }
    pass Shade    { dispatch_material_tiles(Vis64, sun_sky); }
    pass Present  { tonemap_aces(ColorHdr, sky_color); }
}
```
