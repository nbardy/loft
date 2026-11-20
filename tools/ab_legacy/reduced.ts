// Fixture-specific legacy composition. Use the native shelf's fixed roughness,
// dielectric GGX, sky quadrature, local SDF AO and display curve.
const { find, lighting, raster, resolve, screenDraw, shadePixel, world, write } =
  await import(process.env.LOFT_AB_LEGACY + "/src/render/index.ts");

const energyFile = new URL("../../crates/loft-passes/src/bsdf/energy.in", import.meta.url);
const energy = (await Bun.file(energyFile).text()).match(/\d+/g)!.map(Number);
const row = Math.floor(Math.fround(0.55) * 32);
if (energy.length !== 33 * 34) throw new Error("unexpected native GGX table dimensions");
const words = energy.slice(row * 34, (row + 2) * 34).map(v => v + ".0").join(",");
const reducedShade = `const ab_energy = array<f32, 68>(${words});\n`
  + await Bun.file(new URL("reduced.wgsl", import.meta.url)).text();

const aces = `fn present(c:vec3f)->vec4f {
  let mapped=clamp((c*(2.51*c+0.03))/(c*(2.43*c+0.59)+0.14),vec3f(0),vec3f(1));
  return vec4f(select(1.055*pow(mapped,vec3f(1/2.4))-0.055,12.92*mapped,mapped<=vec3f(0.0031308)),1);
}`;

export function reduced(s: Parameters<typeof world>[0]) {
  const w = world({ ...s, shadows: [] });
  const f = find(w, 1);
  const l = lighting(s, w, 3);
  const shade = shadePixel(1, reducedShade, `fn background(r:Ray)->vec4f {
    let sun=normalize(emitters[0].pos);
    let a=max(dot(r.d,sun),0.0);
    return vec4f(ab_dome(r.d)+vec3f(1,0.85,0.6)*(pow(a,600.0)*6.0+pow(a,8.0)*0.2),1);
  }`) + "fn finish(c:vec4f,t:f32)->vec4f{return c;}";
  return {
    tables: l.tables,
    modules: [f.module, l.module(shade), resolve(aces, false)],
    schedule: {
      images: [...f.images, { format: "Rgba16Float", samples: 1 }],
      passes: [
        f.pass,
        raster("shade", 1, [screenDraw("shade_pixel")], [write({ Image: 2 })], [{ Now: 0 }, { Now: 1 }]),
        raster("film", 2, [screenDraw("finish")], [write("Output")], [{ Now: 2 }]),
      ],
    },
  };
}
