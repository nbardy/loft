# Task 1: hermit crab

```hlsl
struct World : ISpace {};
struct Body : ISpace {};
struct Leg : ISpace {};
struct Shell : ISpace {};
struct Claw : ISpace {};
struct Face : ISpace {};

Xform<Leg, Body> leg_socket(uint j, float side)
{
    float yaw = side * (0.5 + 0.3 * float(j)) * 1.0rad;
    return Xform<Leg, Body>.from_yaw_translate(yaw, Vec<Body>(side * 0.12m, 0.0m, 0.05m * float(j) - 0.12m));
}

Pt<Shell> spine(float u)
{
    float r = 0.05 * exp(-1.6 * u);
    float a = 6.2831853 * 2.5 * u;
    return Pt<Shell>(r * cos(a), 0.8 * u, r * sin(a));
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
struct Whelk : ISurface<Shell>
{
    Xform<Shell, World> pose;
    float rib_count;
    float tube_scale;

    [Grid(256, 24)]
    Pt<Shell> surface(float u, float v)
    {
        Frame<Shell> f = sample_c2<Whorl>(u);
        float rad = tube_scale * (0.004 + 0.02 * u) * (1.0 + 0.25 * cos(rib_count * v));
        return f.origin + rad * (cos(v) * f.normal + sin(v) * f.binormal);
    }
};

[Kind]
struct Crusher : ITrace<Claw>
{
    RowId<CrabBody> parent;
    Xform<Claw, World> pose;
    Radians opening;
    Meters size;
    float teeth;

    Field<Claw> trace(Pt<Claw> p)
    {
        Field<Claw> palm = sd_round_box(p, Vec<Claw>(size, 0.8m * size, 0.5m * size), 0.01m);
        Field<Claw> fixed_finger = sd_capsule(p, Pt<Claw>(size, 0.0, 0.0), Pt<Claw>(2.5m * size, 0.0, -0.2m * size), 0.02m);
        Pt<Claw> hinge = Pt<Claw>(size, 0.0, 0.6m * size);
        Vec<Claw> d = p - hinge;
        float c = cos(opening);
        float s = sin(opening);
        Pt<Claw> q = hinge + Vec<Claw>(c * d.x + s * d.z, d.y, -s * d.x + c * d.z);
        float wave = 0.5 + 0.5 * cos(teeth * (q.x / size));
        Field<Claw> moving = sd_capsule(q, hinge, hinge + Vec<Claw>(2.5m * size, 0.0m, -0.2m * size), 0.018m) - 0.004m * wave;
        return smin_poly(palm, smin_poly(fixed_finger, moving, 0.01m), 0.02m);
    }
};

[Kind]
struct CrabLeg : ITrace<Leg>
{
    RowId<CrabBody> parent;
    Xform<Leg, World> pose;
    Meters femur;
    Meters tibia;
    Pt<World> foot;

    Field<Leg> trace(Pt<Leg> p)
    {
        Pt<Leg> knee = Pt<Leg>(femur, 0.0, 0.0);
        Pt<Leg> toe = knee + Vec<Leg>(tibia * 0.6, -tibia * 0.8, 0.0);
        return smin_poly(sd_capsule(p, Pt<Leg>(0.0, 0.0, 0.0), knee, 0.025m),
                         sd_capsule(p, knee, toe, 0.015m), 0.02m);
    }
};

[Kind]
struct CrabBody : ITrace<Body>, IMaterial<Body>
{
    Xform<Body, World> pose;
    Pt<World> com;
    Vec<World> vel;
    Pt<Body> head;
    Meters radius;

    Field<Body> trace(Pt<Body> p)
    {
        return sd_round_box(p, Vec<Body>(radius, 0.6m * radius, 1.2m * radius), 0.02m);
    }

    Bsdf material(Pt<Body> p, Norm<Body> n)
    {
        float grain = fbm2(float2(p.x, p.z) * 900.0);
        float rough = 0.22 + 0.12 * grain;
        float3 base = float3(0.78, 0.27, 0.12);
        return ggx(base, rough, 0.0) + sss(float3(0.9, 0.45, 0.2), 0.006m) + clearcoat(0.7, 0.04);
    }
};

[Kind]
struct Eye : ITrace<Face>
{
    RowId<CrabBody> parent;
    Xform<Face, World> pose;
    Meters stalk;

    Field<Face> trace(Pt<Face> q, Sec time)
    {
        let sym = map(q).mirror_x().translate(-Vec<Face>(0.15, 0.72, 0.42));
        Pt<Face> eye_c = sym.pt(0.03, 0.30, 0.07);
        Field<Face> globe = sd_sphere(sym.p - eye_c, 0.085m);
        Field<Face> stalk_f = sd_capsule(sym.p, Pt<Face>(0.0, 0.0, 0.0), eye_c, 0.012m);
        Pt<World> target = drift(time);
        Pt<World> eye_w = pose * sym.to_parent(eye_c);
        Dir<Face> look = sym.from_parent(inverse(pose) * normalize(target - eye_w));
        Pt<Face> pupil_c = eye_c + 0.05m * look;
        Field<Face> pupil = sd_sphere(sym.p - pupil_c, 0.03m);
        return smin_poly(ssub_poly(pupil, globe, 0.01m), stalk_f, 0.02m);
    }
};

Pt<World> drift(Sec time)
{
    return Pt<World>(0.4 + 0.1 * sin(0.3 * time), 0.25, 0.8 + 0.05 * cos(0.2 * time));
}

[Kind]
struct Ground : ITrace<World>, IMaterial<World>
{
    Field<World> trace(Pt<World> p)
    {
        return p.y;
    }

    Bsdf material(Pt<World> p, Norm<World> n)
    {
        float grain = fbm2(float2(p.x, p.z) * 60.0);
        return ggx(float3(0.42, 0.36, 0.28) * (0.9 + 0.1 * grain), 0.6, 0.0);
    }
};

[Kind] struct Sun : ILight
{
    Dir<World> dir;
    float3 color;
};

[Kind] struct Sky : ISky {};

[Comptime]
float rnd(uint i, uint k)
{
    uint h = i * 2654435761u + k * 40503u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    return float(h & 0xffffu) / 65535.0;
}

[Comptime]
void crab(Xform<Body, World> body)
{
    RowId<CrabBody> b = emit(CrabBody{ .pose = body, .com = body * Pt<Body>(0.0, 0.0, 0.0),
                                       .vel = Vec<World>(0.0, 0.0, 0.0), .head = Pt<Body>(0.0, 0.05, 0.25),
                                       .radius = 0.06m });
    for (uint j = 0; j < 6; j++)
        for (float side = -1.0; side <= 1.0; side += 2.0)
        {
            float t = float(j) / 5.0;
            emit(CrabLeg{ .parent = b, .pose = body * leg_socket(j, side), .femur = 0.30m + 0.08m * t,
                          .tibia = 0.25m - 0.05m * t, .foot = body * Pt<Body>(side * 0.3, 0.0, 0.3 * t - 0.2) });
        }
    emit(Crusher{ .parent = b, .pose = body, .opening = 0.6rad, .size = 0.09m, .teeth = 18.0 });
    emit(Crusher{ .parent = b, .pose = body, .opening = 0.4rad, .size = 0.05m, .teeth = 12.0 });
    emit(Eye{ .parent = b, .pose = body, .stalk = 0.05m });
}

[Comptime]
void crabs()
{
    crab(Xform<Body, World>.from_yaw_translate(0.0rad, Vec<World>(0.0, 0.0, 0.0)));
    emit(Whelk{ .pose = Xform<Shell, World>.from_yaw_translate(0.0rad, Vec<World>(-0.25, 0.0, 0.1)),
                .rib_count = 9.0, .tube_scale = 1.0 });
}

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

[Constraint]
Verdict<Meters> upright(CrabBody b)
{
    if (!is_finite(b.pose)) return Verdict::Unknown(NonFinitePose);
    Meters margin = (b.pose * b.head).y - 0.15m;
    if (margin < 0.0m) return Verdict::Missed(-margin);
    return Verdict::Held(margin);
}

technique Main
{
    pass Rows     { crabs(); }
    pass Motion   { run_laws(body_law, crab_step); }
    pass Check    { check(upright); }
    pass Camera   { orbit(Pt<World>(0.0, 0.4, 0.0), 6m, 40deg); }
    pass Cull     { build_indirect<Ground, CrabLeg, Crusher, Whelk, Eye>(); }
    pass Vis      { render_bundle(Vis64) { draw_indirect<Ground, CrabLeg, Crusher, Whelk, Eye, CrabBody>(); } }
    pass Classify { tile_bin_kinds(Vis64, 8); }
    pass Shade    { dispatch_material_tiles(Vis64, sun_sky); }
    pass Present  { tonemap_aces(ColorHdr, sky_color); }
}
```

# Task 2: reed bed in wind

```hlsl
struct World : ISpace {};

[Comptime]
float rnd(uint i, uint k)
{
    uint h = i * 2654435761u + k * 40503u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    return float(h & 0xffffu) / 65535.0;
}

float gust_at(Pt<World> root, Sec time)
{
    return 0.35 + 0.25 * sin(0.9 * root.x - 1.6 * time) + 0.1 * sin(2.3 * root.z - 0.7 * time);
}

[Kind]
struct Reed : ITrace<World>, IMaterial<World>
{
    Pt<World> root;
    Meters height;
    Meters lean;
    Meters head;
    float stiffness;
    float phase;

    Pt<World> stalk_at(float s, Sec time)
    {
        float a = gust_at(root, time) * (1.0 - 0.7 * stiffness) + 0.1 * sin(phase + 3.0 * time);
        return root + Vec<World>(lean * s + height * a * s * s, height * s, 0.0m);
    }

    Field<World> trace(Pt<World> p, Sec time)
    {
        Field<World> d = 1.0e3m;
        Pt<World> a = root;
        for (uint i = 1; i <= 6; i++)
        {
            float s = float(i) / 6.0;
            Pt<World> b = stalk_at(s, time);
            d = min(d, sd_capsule(p, a, b, 0.012m - 0.008m * s));
            a = b;
        }
        Pt<World> tip = stalk_at(1.0, time);
        return smin_poly(d, sd_sphere(p - tip, head), 0.01m);
    }

    Bsdf material(Pt<World> p, Norm<World> n)
    {
        return ink(float3(0.14, 0.12, 0.16), 0.0);
    }
};

[Kind]
struct Water : ITrace<World>, IMaterial<World>
{
    Field<World> trace(Pt<World> p)
    {
        return p.y;
    }

    Bsdf material(Pt<World> p, Norm<World> n)
    {
        float3 paper = float3(0.93, 0.89, 0.78);
        float speck = fbm2(float2(p.x, p.z) * 30.0);
        return emission(paper - 0.05 * speck * float3(1.0, 0.9, 0.6));
    }
};

[Kind] struct PaperSky : ISky {};

[Lobe] float4 ink(float3 dark, float cut, float3 n, float3 v, float3 l)
{
    return float4(dot(n, l) < cut ? dark : float3(0.93, 0.89, 0.78), 0.0);
}

[Comptime]
void bed()
{
    for (uint i = 0; i < 200; i++)
    {
        float cx = 3.0 * (rnd(i, 1) - 0.5) * (0.6 + 0.4 * rnd(i, 2));
        float cz = 2.0 * (rnd(i, 3) - 0.5);
        float stiff = rnd(i, 4);
        emit(Reed{ .root = Pt<World>(cx, 0.0, cz),
                   .height = 0.6m + 0.5m * rnd(i, 5),
                   .lean = (rnd(i, 6) - 0.5) * 0.08m,
                   .head = 0.03m + 0.02m * rnd(i, 7),
                   .stiffness = stiff,
                   .phase = 6.2831853 * rnd(i, 8) });
    }
}

technique Main
{
    pass Rows     { bed(); emit(Water{}); }
    pass Camera   { orbit(Pt<World>(0.0, 0.5, 0.0), 4m, 25deg); }
    pass Cull     { build_indirect<Water, Reed>(); }
    pass Vis      { render_bundle(Vis64) { draw_indirect<Water, Reed>(); } }
    pass Classify { tile_bin_kinds(Vis64, 8); }
    pass Shade    { dispatch_material_tiles(Vis64, paper_sky); }
    pass Present  { tonemap_aces(ColorHdr, paper_sky); }
}
```

# Task 3: heron's hop

```hlsl
struct World : ISpace {};
struct Body : ISpace {};
struct Leg : ISpace {};
struct NeckSpace : ISpace {};
struct Head : ISpace {};

[Kind]
struct Fish : ITrace<World>, IMaterial<World>
{
    Pt<World> pos;
    Sec clock;

    Field<World> trace(Pt<World> p)
    {
        return sd_sphere(p - pos, 0.05m);
    }

    Bsdf material(Pt<World> p, Norm<World> n)
    {
        return ggx(float3(0.6, 0.65, 0.7), 0.3, 0.0) + emission(float3(0.02, 0.03, 0.04));
    }
};

Pt<World> fish_path(Sec t)
{
    return Pt<World>(2.0 + 0.6 * sin(0.5 * t), 0.0, 1.5 + 0.8 * cos(0.31 * t));
}

[Kind]
struct Heron : ITrace<Body>, IMaterial<Body>
{
    Pt<World> pos;
    Radians yaw;
    Radians neck_yaw;
    Radians neck_limit;
    Radians gaze_rel;
    Meters lift;
    Meters hop_height;
    Sec hop_t;
    bool hopping;
    uint target;
    Xform<Body, World> pose;

    Field<Body> trace(Pt<Body> p)
    {
        return sd_round_box(p, Vec<Body>(0.12m, 0.08m, 0.25m), 0.03m);
    }

    Bsdf material(Pt<Body> p, Norm<Body> n)
    {
        return ggx(float3(0.55, 0.58, 0.6), 0.5, 0.0);
    }
};

[Kind]
struct HeronLeg : ITrace<Leg>
{
    RowId<Heron> parent;
    Xform<Leg, World> pose;
    Meters thigh;
    Meters shank;

    Field<Leg> trace(Pt<Leg> p)
    {
        Pt<Leg> knee = Pt<Leg>(thigh, 0.0, 0.0);
        Pt<Leg> ankle = knee + Vec<Leg>(-0.2m * shank, -shank, 0.0);
        return smin_poly(sd_capsule(p, Pt<Leg>(0.0, 0.0, 0.0), knee, 0.02m),
                         sd_capsule(p, knee, ankle, 0.012m), 0.01m);
    }
};

[Kind]
struct Neck : ITrace<NeckSpace>
{
    RowId<Heron> parent;
    Xform<NeckSpace, Body> pose;
    Meters length;

    Field<NeckSpace> trace(Pt<NeckSpace> p)
    {
        Field<NeckSpace> d = 1.0e3m;
        Pt<NeckSpace> a = Pt<NeckSpace>(0.0, 0.0, 0.0);
        for (uint i = 1; i <= 8; i++)
        {
            float s = float(i) / 8.0;
            float bow = 0.06 * sin(2.0 * 3.14159265 * s);
            Pt<NeckSpace> b = Pt<NeckSpace>(bow * length, s * length, 0.0);
            d = min(d, sd_capsule(p, a, b, 0.02m - 0.01m * s));
            a = b;
        }
        return d;
    }
};

[Kind]
struct Beak : ITrace<Head>
{
    RowId<Heron> parent;
    Xform<Head, Body> pose;
    Meters length;

    Field<Head> trace(Pt<Head> p)
    {
        return sd_capsule(p, Pt<Head>(0.0, 0.0, 0.0), Pt<Head>(0.0, 0.0, length), 0.01m);
    }
};

[Law(Writes = Fish)]
void fish_law(inout Fish f, Sec dt)
{
    f.clock = f.clock + dt;
    f.pos = fish_path(f.clock);
}

[Law(Writes = Heron)]
void body_law(inout Heron h, Table<Fish> fish, Sec dt)
{
    Pt<World> target = fish[h.target].pos;
    Radians want = atan2(target.z - h.pos.z, target.x - h.pos.x);
    Radians rel = wrap_angle(want - h.yaw);
    if (h.hopping)
    {
        h.hop_t = h.hop_t + dt;
        float u = h.hop_t / 0.6s;
        h.lift = h.hop_height * sin(3.14159265 * u);
        h.yaw = h.yaw + (rel > 0.0rad ? 2.0rad : -2.0rad) * dt / 1.0s;
        if (u >= 1.0)
        {
            h.hopping = false;
            h.lift = 0.0m;
        }
    }
    else if (abs(rel) > h.neck_limit)
    {
        h.hopping = true;
        h.hop_t = 0.0s;
    }
    h.pose = Xform<Body, World>.from_yaw_translate(h.yaw, h.pos + Vec<World>(0.0m, h.lift, 0.0m));
}

[Law(Writes = Heron)]
void neck_law(inout Heron h, Table<Fish> fish)
{
    Pt<World> target = fish[h.target].pos;
    Pt<World> eye = h.pose * Pt<Body>(0.0, 0.3, 0.2);
    Radians want = atan2(target.z - eye.z, target.x - eye.x);
    Radians rel = wrap_angle(want - h.yaw);
    h.gaze_rel = rel;
    h.neck_yaw = clamp(rel, -h.neck_limit, h.neck_limit);
}

[Constraint]
Verdict<Radians> gaze_held(Heron h)
{
    Radians leftover = abs(h.gaze_rel) - h.neck_limit;
    if (leftover > 0.0rad) return Verdict::Missed(leftover);
    return Verdict::Held(-leftover);
}

[Constraint]
Verdict<Meters> head_above_hips(Heron h)
{
    if (!is_finite(h.pose)) return Verdict::Unknown(NonFinitePose);
    Meters margin = (h.pose * Pt<Body>(0.0, 0.35, 0.2)).y - (h.pose * Pt<Body>(0.0, 0.0, 0.0)).y;
    if (margin < 0.0m) return Verdict::Missed(-margin);
    return Verdict::Held(margin);
}

[Comptime]
void heron_scene()
{
    emit(Fish{ .pos = fish_path(0.0s), .clock = 0.0s });
    RowId<Heron> h = emit(Heron{ .pos = Pt<World>(0.0, 0.0, 0.0), .yaw = 0.0rad, .neck_yaw = 0.0rad,
                                 .neck_limit = 1.1rad, .gaze_rel = 0.0rad, .lift = 0.0m, .hop_height = 0.08m,
                                 .hop_t = 0.0s, .hopping = false, .target = 0, .pose = Xform<Body, World>.identity() });
    emit(HeronLeg{ .parent = h, .pose = Xform<Leg, World>.identity(), .thigh = 0.35m, .shank = 0.4m });
    emit(HeronLeg{ .parent = h, .pose = Xform<Leg, World>.identity(), .thigh = 0.35m, .shank = 0.4m });
    emit(Neck{ .parent = h, .pose = Xform<NeckSpace, Body>.identity(), .length = 0.5m });
    emit(Beak{ .parent = h, .pose = Xform<Head, Body>.identity(), .length = 0.18m });
}

[Kind] struct Ground : ITrace<World>, IMaterial<World>
{
    Field<World> trace(Pt<World> p)
    {
        return p.y;
    }

    Bsdf material(Pt<World> p, Norm<World> n)
    {
        return ggx(float3(0.42, 0.36, 0.28), 0.6, 0.0);
    }
};

[Kind] struct Sun : ILight
{
    Dir<World> dir;
    float3 color;
};

[Kind] struct Sky : ISky {};

technique Main
{
    pass Rows     { heron_scene(); }
    pass Body     { run_laws(body_law, fish_law); }
    pass Neck     { run_laws(neck_law); }
    pass Check    { check(gaze_held); check(head_above_hips); }
    pass Camera   { orbit(Pt<World>(0.0, 0.5, 0.0), 3m, 30deg); }
    pass Cull     { build_indirect<Ground, Fish, Heron, HeronLeg, Neck, Beak>(); }
    pass Vis      { render_bundle(Vis64) { draw_indirect<Ground, Fish, Heron, HeronLeg, Neck, Beak>(); } }
    pass Classify { tile_bin_kinds(Vis64, 8); }
    pass Shade    { dispatch_material_tiles(Vis64, sun_sky); }
    pass Present  { tonemap_aces(ColorHdr, sky_color); }
}
```
