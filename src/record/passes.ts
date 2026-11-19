// Wire 5 declaration span. Resource and module references are local indices.
export type Image = { format: "Rgba16Uint" | "Depth32Float" | "Rgba16Float" | "Rgba32Float" | "Rgba8Unorm" | "R32Float"; samples: 1 | 4 };
export type Read = { Now: number } | { Previous: number };
export type Write = { target: { Image: number } | "Output"; clear: { Color: [number, number, number, number] } | "Load"; blend: "Opaque" | "Add" | "Alpha" };
export type Draw = { vertex: string; fragment: string; vertices: number; instances: { Fixed: number } | { Rows: number } | { Indirect: number }; cull: "None" | "Back" | "Front" };
export type Pass = {
  name: string; module: number; reads: Read[]; writes: Write[]; repeat: number;
  run: { Raster: { draws: Draw[] } } | { Compute: { entry: string; extent: [number, number, number] } };
};
export type Open = { signature: string; selector: string; cases: [number, string][]; otherwise: string };
export type Source = { text: string; opens: Open[] };
export type Declarations = { tables: ({ lanes: number; words: number[] } | { lanes:number; rows:number; module:number; entry:string; extent:[number,number,number] })[]; modules: Source[]; schedule: { images: Image[]; passes: Pass[] } };
