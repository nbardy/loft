// Port one frozen Loft source without running it. Resolve local bindings in an isolated TS program;
// retain the original source outside the AST spans that actually need editing.
import ts from "typescript-ast";
import { isAbsolute, relative, resolve } from "node:path";
import { existsSync } from "node:fs";
import { exportsOf } from "../src/cli/catalogue.ts";
import { parseModule, ParseError } from "../src/text/parse.ts";
import { wgsl, WGSL_STATE } from "../src/text/index.ts";
import { SceneError } from "../src/errors.ts";

type Reason =
  | "typescript-syntax"
  | "loft-import-path"
  | "condition-options"
  | "clearance-window"
  | "stops-at-contact-and-span"
  | "legacy-hold-type"
  | "raw-wgsl"
  | "drawable-forms"
  | "scene-lights"
  | "point-instancer"
  | "slot-time"
  | "cellular-result"
  | "creative-scene";

type Unported = { line: number; reason: Reason };
type Result = { code: string; changed: boolean; unported: Unported[] };
type Edit = { start: number; end: number; text: string };
type Module = { path: string; file: string; archived: boolean };
type Import = { node: ts.ImportDeclaration; module: Module; named: ts.NamedImports | undefined };
type Binding = { name: string; imported: Import; specifier?: ts.ImportSpecifier };

const root = resolve(import.meta.dir, "..");
const publicNames = exportsOf(resolve(root, "src/index.ts"));
const layers = new Set(["core", "shapes", "motion", "actions", "checks", "styles"]);
const renamed = new Map([["Points", "Poses"], ["Instanceable", "Poseable"], ["SpecksOptions", "PointsOptions"]]);
const rewritten = new Map<string, Reason>([
  ["atLeast", "condition-options"], ["staysClear", "clearance-window"], ["wgslFn", "raw-wgsl"],
]);
const manual = new Map<string, Reason>([
  ["stopsAt", "stops-at-contact-and-span"], ["When", "legacy-hold-type"],
  ["WgslFnSpec", "raw-wgsl"], ["Bakeable", "drawable-forms"],
  ["Flash", "scene-lights"], ["flash", "scene-lights"],
]);
const screen = new Map([
  ["fwidth", "fn pixel_rate(x: f32) -> f32 { return fwidth(x); }"],
  ["viewDir", "fn toward_point(p: vec3f) -> vec3f { return view_dir(p); }"],
  ["eyePosition", "fn eye_point() -> vec3f { return frame.eye.xyz; }"],
  ["pixelWorld", "fn pixel_size(p: vec3f) -> f32 { return pixel_world(p); }"],
]);
const known = (name: string) => publicNames.has(name) || renamed.has(name) || rewritten.has(name) || manual.has(name) || screen.has(name);

function moduleOf(path: string): Module | undefined {
  const part = path.slice("loft/".length);
  if (path === "loft") return { path, file: "index.ts", archived: false };
  if (path.startsWith("loft/") && layers.has(part)) return { path, file: part + ".ts", archived: false };
  const archive = /(?:^|\/)(?:frozen-loft|training\/loft\/library)\/src\/(.+\.ts)$/.exec(path);
  const file = archive?.[1]
    ?? /^(?:(?:\.\.?\/)+|.*\/(?:loft|loft-migration)\/)src\/(.+\.ts)$/.exec(path)?.[1];
  return file === undefined ? undefined : { path, file, archived: archive !== null };
}

function unwrap(node: ts.Expression): ts.Expression {
  while (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isTypeAssertionExpression(node)
      || ts.isSatisfiesExpression(node) || ts.isNonNullExpression(node)) node = node.expression;
  return node;
}

function property(node: ts.PropertyName): string | undefined {
  return ts.isIdentifier(node) || ts.isStringLiteral(node) ? node.text : undefined;
}

/** Read simple named fields once; duplicates, getters, computed keys and spreads need a person. */
function fields(node: ts.Expression): Map<string, ts.Expression> | undefined {
  const e = unwrap(node);
  if (!ts.isObjectLiteralExpression(e)) return undefined;
  const out = new Map<string, ts.Expression>();
  for (const p of e.properties) {
    const key = p.name && property(p.name);
    if (key === undefined || out.has(key)) return undefined;
    if (ts.isPropertyAssignment(p)) out.set(key, p.initializer);
    else if (ts.isShorthandPropertyAssignment(p) && !p.objectAssignmentInitializer) out.set(key, p.name);
    else return undefined;
  }
  return out;
}

function access(node: ts.Node): { base: ts.Expression; key: ts.Node; name: string } | undefined {
  if (ts.isPropertyAccessExpression(node)) return { base: node.expression, key: node.name, name: node.name.text };
  if (ts.isElementAccessExpression(node) && ts.isStringLiteral(node.argumentExpression))
    return { base: node.expression, key: node.argumentExpression, name: node.argumentExpression.text };
  return undefined;
}

function staticText(node: ts.Expression | undefined): string | undefined {
  if (node === undefined) return undefined;
  const e = unwrap(node);
  return ts.isStringLiteral(e) || ts.isNoSubstitutionTemplateLiteral(e) ? e.text : undefined;
}

function syntaxOf(code: string, filename: string) {
  const options: ts.CompilerOptions = { target: ts.ScriptTarget.Latest, noResolve: true, noLib: true, types: [], strict: true };
  const file = ts.createSourceFile(filename, code, options.target!, true, ts.ScriptKind.TS);
  const host = ts.createCompilerHost(options);
  host.getSourceFile = (name) => name === filename ? file : undefined;
  const program = ts.createProgram([filename], options, host);
  return { file, checker: program.getTypeChecker(), syntax: program.getSyntacticDiagnostics(file) };
}

/** The input and output filenames let file mode preserve a deep import relative to the output. */
export function migrate(code: string, filename = "scene.ts", output: string | null = filename): Result {
  const { file, checker, syntax } = syntaxOf(code, filename);
  const diagnostics = new Map<string, Unported>();
  const edits: Edit[] = [];
  const lineAt = (pos: number) => file.getLineAndCharacterOfPosition(pos).line + 1;
  function report(node: ts.Node, reason: Reason) {
    const line = lineAt(node.getStart(file));
    diagnostics.set(line + ":" + reason, { line, reason });
  }
  function result(next: string): Result {
    return { code: next, changed: next !== code, unported: [...diagnostics.values()].sort((a, b) => a.line - b.line) };
  }
  if (syntax.length > 0) {
    for (const d of syntax) {
      const line = lineAt(d.start ?? 0);
      diagnostics.set(String(line), { line, reason: "typescript-syntax" });
    }
    return result(code);
  }

  function replace(start: number, end: number, text: string) {
    // A parent replacement uses the already-edited text of its children.
    for (let i = edits.length - 1; i >= 0; i--) {
      const e = edits[i]!;
      if (e.start >= start && e.end <= end) edits.splice(i, 1);
      else if (e.start < end && start < e.end) throw new Error("overlapping migration edits");
    }
    edits.push({ start, end, text });
  }
  const set = (node: ts.Node, text: string) => replace(node.getStart(file), node.end, text);
  function source(start: number, end: number) {
    let patch = code.slice(start, end);
    for (const e of edits.filter((e) => e.start >= start && e.end <= end).sort((a, b) => b.start - a.start)) {
      patch = patch.slice(0, e.start - start) + e.text + patch.slice(e.end - start);
    }
    return patch;
  }
  const read = (node: ts.Node) => source(node.getStart(file), node.end);
  const symbol = (node: ts.Node) => ts.isShorthandPropertyAssignment(node.parent)
    ? checker.getShorthandAssignmentValueSymbol(node.parent)
    : ts.isExportSpecifier(node.parent) ? checker.getExportSpecifierLocalTargetSymbol(node.parent)
    : checker.getSymbolAtLocation(node);
  const nodes: ts.Node[] = [];
  const identifiers = new Set<string>();
  const scan = (node: ts.Node) => {
    if (ts.isIdentifier(node)) {
      const p = node.parent;
      const propertyName = (ts.isPropertyAssignment(p) || ts.isPropertyAccessExpression(p)) && p.name === node;
      const importedName = ts.isImportSpecifier(p) && p.propertyName === node;
      if (!propertyName && !importedName) identifiers.add(node.text);
    }
    ts.forEachChild(node, scan);
    nodes.push(node);
  };
  scan(file);

  const imports: Import[] = [];
  const bindings = new Map<ts.Symbol, Binding>();
  const references = new Map<ts.Symbol, ts.Identifier[]>();
  const used = new Set<ts.Node>();
  const refused = new Set<ts.Node>();
  const helpers = new Map<string, string>();
  const declarations: string[] = [];
  const screenBindings = new Map<string, string>();
  function bind(local: ts.Identifier, binding: Binding) {
    const s = symbol(local);
    if (s) bindings.set(s, binding);
  }
  for (const node of file.statements) {
    if (!ts.isImportDeclaration(node) || !ts.isStringLiteral(node.moduleSpecifier)) continue;
    const module = moduleOf(node.moduleSpecifier.text);
    if (!module) continue;
    const names = node.importClause?.namedBindings;
    const named = names && ts.isNamedImports(names) ? names : undefined;
    if (!module.archived && !module.path.startsWith("loft") && named
        && !named.elements.some((e) => known((e.propertyName ?? e.name).text))) continue;
    const imported = { node, module, named };
    imports.push(imported);
    if (names && ts.isNamespaceImport(names)) bind(names.name, { name: "*", imported });
    for (const e of named?.elements ?? []) bind(e.name, { name: (e.propertyName ?? e.name).text, imported, specifier: e });
  }
  function binding(node: ts.Node): Binding | undefined {
    const s = symbol(node);
    return s && bindings.get(s);
  }
  function localConst(node: ts.Expression): ts.VariableDeclaration | undefined {
    if (!ts.isIdentifier(node)) return undefined;
    const d = symbol(node)?.valueDeclaration;
    return d && ts.isVariableDeclaration(d) && ts.isVariableDeclarationList(d.parent)
      && (d.parent.flags & ts.NodeFlags.Const) !== 0 ? d : undefined;
  }
  function valueOf(node: ts.Expression): ts.Expression {
    const expr = unwrap(node), init = localConst(expr)?.initializer;
    return init ? unwrap(init) : expr;
  }
  function usedOnlyHere(node: ts.Expression, decl: ts.VariableDeclaration) {
    const local = symbol(node);
    const statement = decl.parent.parent;
    return !decl.type && !(ts.isVariableStatement(statement) && statement.modifiers?.length)
      && !nodes.some((n) => ts.isIdentifier(n) && n !== decl.name && n !== node && symbol(n) === local);
  }
  function api(node: ts.Expression): Binding | undefined {
    const e = unwrap(node);
    if (ts.isIdentifier(e)) return binding(e);
    const m = access(e);
    const ns = m && binding(m.base);
    if (ns?.name === "*") return { name: m!.name, imported: ns.imported };
    return undefined;
  }
  for (const node of nodes) {
    if (!ts.isIdentifier(node) || ts.isImportSpecifier(node.parent)) continue;
    const s = symbol(node);
    if (s && bindings.has(s)) {
      const refs = references.get(s) ?? [];
      refs.push(node);
      references.set(s, refs);
    }
  }

  function fresh(name: string) {
    let local = name;
    while (identifiers.has(local)) local = local === name ? "loft" + name[0]!.toUpperCase() + name.slice(1) : local + "_";
    identifiers.add(local);
    return local;
  }
  function helper(name: string, at: ts.Node): string {
    // Use an existing import only if it resolves to that binding at the insertion site.
    for (const [sym, b] of bindings) {
      const spec = b.specifier;
      if (b.name === name && spec && !spec.isTypeOnly && !b.imported.node.importClause?.isTypeOnly
          && checker.resolveName(spec.name.text, at, ts.SymbolFlags.Value | ts.SymbolFlags.Alias, true) === sym) return spec.name.text;
    }
    const prior = helpers.get(name);
    if (prior !== undefined) return prior;
    const local = fresh(name);
    helpers.set(name, local);
    return local;
  }
  function use(node: ts.Expression) {
    used.add(unwrap(node));
  }
  function refuse(call: ts.CallExpression, reason: Reason) {
    refused.add(unwrap(call.expression));
    report(call, reason);
  }
  function numeric(node: ts.Expression) {
    return (checker.getTypeAtLocation(node).flags & ts.TypeFlags.NumberLike) !== 0;
  }
  function opted(node: ts.Expression | undefined, fallback: string) {
    return node === undefined ? fallback : read(node);
  }
  function window(at: ts.CallExpression, lo: string, hi: string, condition: string, settings = "") {
    return helper("always", at) + "([" + helper("time", at) + ", " + lo + ", " + hi + "], " + condition + settings + ")";
  }

  function condition(call: ts.CallExpression, name: "atLeast" | "equal") {
    const [a, b, options] = call.arguments;
    const absent = options === undefined || isUndefined(unwrap(options));
    if (name === "equal" && (absent || (options && numeric(options)))) return;
    const o = absent ? new Map<string, ts.Expression>() : fields(options!);
    const at = o?.get("at"), from = o?.get("from"), to = o?.get("to"), within = o?.get("within");
    if (!a || !b || call.arguments.length > 3 || [a, b].some(ts.isSpreadElement) || !o
        || [...o.keys()].some((k) => !["at", "from", "to", ...(name === "equal" ? ["within"] : [])].includes(k))
        || (at ? from !== undefined || to !== undefined || !numeric(at) || ts.isCallExpression(unwrap(at)) : Boolean(from) !== Boolean(to))
        || (!absent && !at && !from && !within)) {
      refuse(call, "condition-options");
      return;
    }
    const test = (name === "equal" ? read(call.expression) : helper("ge", call)) + "(" + read(a) + ", " + read(b)
      + (within ? ", " + read(within) : "") + ")";
    set(call, at ? window(call, read(at), read(at), test) : from && to ? window(call, read(from), read(to), test) : test);
    use(call.expression);
  }
  function isUndefined(node: ts.Expression) {
    return ts.isIdentifier(node) && node.text === "undefined"
      && (checker.getTypeAtLocation(node).flags & ts.TypeFlags.Undefined) !== 0;
  }

  function clearance(call: ts.CallExpression) {
    const [body, target, options] = call.arguments;
    const o = options && fields(options);
    const from = o?.get("from"), until = o?.get("until"), rate = o?.get("rate"), label = o?.get("label");
    if (!body || !target || call.arguments.length !== 3 || [body, target].some(ts.isSpreadElement) || !o || !until
        || [...o.keys()].some((k) => !["from", "until", "rate", "label", "margin"].includes(k))
        || (rate && read(rate) !== "60") || !numeric(until) || (from && !numeric(from))) {
      refuse(call, "clearance-window");
      return;
    }
    const gap = helper("clearance", call) + "(" + read(body) + ", " + read(target) + ")";
    const test = helper("ge", call) + "(" + gap + ", " + opted(o.get("margin"), "0") + ")";
    set(call, window(call, opted(from, "0"), read(until), test, label ? ", { label: " + read(label) + " }" : ""));
    use(call.expression);
  }

  function rawFunction(call: ts.CallExpression) {
    const arg = call.arguments[0];
    const o = arg && fields(arg);
    const name = staticText(o?.get("name")), text = staticText(o?.get("wgsl")), ret = staticText(o?.get("ret"));
    if (call.arguments.length !== 1 || !o || [...o.keys()].some((k) => !["name", "wgsl", "ret"].includes(k))
        || !name || !text || !ret) {
      refuse(call, "raw-wgsl");
      return;
    }
    try {
      const { fns, structs, consts, raws } = parseModule(text);
      const f = fns[0];
      if (fns.length !== 1 || structs.length || consts.length || raws.length || f?.kind !== "raw" || f.sig.name !== name
          || wgsl(text)[WGSL_STATE].raw(name).ret !== ret) {
        refuse(call, "raw-wgsl");
        return;
      }
    } catch (e) {
      if (!(e instanceof ParseError || e instanceof SceneError)) throw e;
      refuse(call, "raw-wgsl");
      return;
    }
    set(call, helper("wgsl", call) + "(" + JSON.stringify(text) + ")[" + JSON.stringify(name) + "]!");
    use(call.expression);
  }

  const editedSlots = new Set<ts.Node>();
  function slot(node: ts.Expression) {
    const expr = unwrap(node);
    const fn = valueOf(node);
    if (editedSlots.has(fn)) return;
    if (!ts.isArrowFunction(fn) && !ts.isFunctionExpression(fn)) {
      if (checker.getTypeAtLocation(expr).getCallSignatures().some((sig) => sig.parameters.length >= 2)) report(expr, "slot-time");
      return;
    }
    const [first, last, ...extra] = fn.parameters;
    if (!last) return;
    const d = localConst(expr);
    if (d && !usedOnlyHere(expr, d)) {
      report(expr, "slot-time");
      return;
    }
    if (extra.length || !ts.isIdentifier(last.name) || last.initializer || last.dotDotDotToken || first?.dotDotDotToken
        || !fn.body || nodes.some((n) => ts.isIdentifier(n) && n.text === "arguments" && n.pos >= fn.body.pos && n.end <= fn.body.end)) {
      report(fn, "slot-time");
      return;
    }
    const local = symbol(last.name);
    const refs = nodes.filter((n): n is ts.Identifier => ts.isIdentifier(n) && n !== last.name &&
      n.pos >= fn.body.pos && n.end <= fn.body.end && local !== undefined && symbol(n) === local);
    // A write to the old parameter needs a local variable and a decision about when it is read.
    if (refs.some((ref) => !isRead(ref))) {
      report(fn, "slot-time");
      return;
    }
    for (const ref of refs) {
      const time = helper("time", ref);
      set(ref, ts.isShorthandPropertyAssignment(ref.parent) ? ref.text + ": " + time : time);
    }
    editedSlots.add(fn);
    replace(first!.end, last.end, "");
  }

  function isRead(ref: ts.Identifier): boolean {
    let place: ts.Node = ref;
    while (ts.isParenthesizedExpression(place.parent) || ts.isArrayLiteralExpression(place.parent)
        || ts.isPropertyAssignment(place.parent) || ts.isShorthandPropertyAssignment(place.parent)
        || ts.isObjectLiteralExpression(place.parent)) place = place.parent;
    const p = place.parent;
    if (ts.isBinaryExpression(p) && p.left === place && p.operatorToken.kind >= ts.SyntaxKind.FirstAssignment
        && p.operatorToken.kind <= ts.SyntaxKind.LastAssignment) return false;
    if ((ts.isPrefixUnaryExpression(p) || ts.isPostfixUnaryExpression(p))
        && (p.operator === ts.SyntaxKind.PlusPlusToken || p.operator === ts.SyntaxKind.MinusMinusToken)) return false;
    return !(ts.isForOfStatement(p) || ts.isForInStatement(p)) || p.initializer !== place;
  }

  function drawCall(call: ts.CallExpression, name: string, key: ts.Node, module: Module) {
    const args = call.arguments;
    if (name === "points" && args.length === 3) {
      // Both point APIs accept three arguments, but only the speck API takes a number second.
      const points = valueOf(args[1]!);
      const placement = ts.isCallExpression(points) && ["points", "posed", "posedList"].includes(api(points.expression)?.name ?? "");
      const constructor = valueOf(args[2]!);
      const callback = ts.isArrowFunction(constructor) || ts.isFunctionExpression(constructor)
        || checker.getTypeAtLocation(constructor).getCallSignatures().length > 0;
      if (args.some(ts.isSpreadElement)) report(call, "point-instancer");
      else if (placement || (module.archived && !numeric(points) && !callback)) {
        set(key, ts.isStringLiteral(key) ? JSON.stringify("posed") : "posed");
      } else if (numeric(points) || callback) slot(args[2]!);
      else report(call, "point-instancer");
    } else if (name === "specks" || (name === "points" && args.length === 4)) {
      if (name === "specks") set(key, ts.isStringLiteral(key) ? JSON.stringify("points") : "points");
      if (args[2]) slot(args[2]);
    } else if (name === "volume") {
      if (args[1]) slot(args[1]);
      if (args[2]) {
        const expr = unwrap(args[2]), d = localConst(expr);
        const options = d && !usedOnlyHere(expr, d) ? undefined : fields(valueOf(expr));
        for (const k of ["albedo", "emission"]) {
          const opt = options?.get(k);
          if (opt) slot(opt);
        }
        if (!options) report(args[2], "slot-time");
      }
    }
  }

  function propertyName(key: ts.Node, name: string) {
    set(key, ts.isStringLiteral(key) ? JSON.stringify(name) : name);
  }
  function typeName(type: ts.TypeNode | undefined) {
    if (!type || !ts.isTypeReferenceNode(type)) return undefined;
    if (ts.isIdentifier(type.typeName)) return binding(type.typeName)?.name;
    return binding(type.typeName.left)?.name === "*" ? type.typeName.right.text : undefined;
  }

  // These edits can sit inside a call that is itself replaced, so do them before replacing calls.
  for (const node of nodes) {
    if (ts.isObjectLiteralExpression(node)) {
      const p = node.parent;
      const t = ts.isVariableDeclaration(p) || ts.isAsExpression(p) || ts.isSatisfiesExpression(p) ? p.type : undefined;
      if (typeName(t) === "Drawable") {
        const surfaces = node.properties.find((prop) => prop.name && property(prop.name) === "surfaces");
        if (surfaces) {
          const init = ts.isPropertyAssignment(surfaces) ? unwrap(surfaces.initializer) : undefined;
          if (init && ts.isArrayLiteralExpression(init) && init.elements.length === 0) propertyName(surfaces.name!, "forms");
          else report(surfaces, "drawable-forms");
        }
      }
      if (["Points", "Instanceable"].includes(typeName(t) ?? "")) report(node, "point-instancer");
    }
    const m = access(node);
    if (!m) continue;
    if (m.name === "surfaces" && ["plane", "nothing"].includes(api(m.base)?.name ?? "")) propertyName(m.key, "forms");
    const ns = binding(m.base);
    if (ns?.name === "*" && screen.has(m.name)) {
      let local = screenBindings.get(m.name);
      if (local === undefined) {
        local = fresh(m.name);
        screenBindings.set(m.name, local);
        addScreen(m.name, local);
      }
      set(node, local);
    }
    const name = ns?.name === "*" ? renamed.get(m.name) : undefined;
    if (name) propertyName(m.key, name);
    if (m.name === "instanced") {
      const d = symbol(m.base)?.valueDeclaration;
      const t = d && (ts.isVariableDeclaration(d) || ts.isParameter(d)) ? d.type : undefined;
      const explicit = typeName(t) === "Instanceable";
      const base = valueOf(m.base);
      const ctor = ts.isCallExpression(base) ? access(base.expression) : undefined;
      const strand = ts.isCallExpression(base) && api(base.expression)?.name === "strand";
      if (explicit || strand || (ctor && api(ctor.base)?.name === "draw" && ["surface", "glazed"].includes(ctor.name)))
        propertyName(m.key, "posed");
      else if (imports.some((i) => i.module.archived)) report(node, "point-instancer");
    }
  }

  // Update slot bindings before rewriting calls inside a callback body.
  for (const node of nodes) {
    if (!ts.isCallExpression(node) || node.questionDotToken) continue;
    const m = access(unwrap(node.expression));
    const owner = m && api(m.base);
    if (m && owner?.name === "draw") {
      drawCall(node, m.name, m.key, owner.imported.module);
      use(node.expression);
    }
  }
  for (const node of nodes) {
    if (!ts.isCallExpression(node) || node.questionDotToken) continue;
    const entry = api(node.expression);
    switch (entry?.name) {
      case "atLeast": case "equal": condition(node, entry.name); break;
      case "staysClear": clearance(node); break;
      case "wgslFn": rawFunction(node); break;
      case "medium": {
        const options = node.arguments[1];
        if (options && fields(valueOf(options))?.has("flashes")) report(options, "scene-lights");
        break;
      }
      case "cellular2": {
        if (!entry.imported.module.archived) break;
        const outer = access(node.parent);
        if (!outer || outer.base !== node || !["x", "y", "xy"].includes(outer.name)) set(node, read(node) + ".xy");
        use(node.expression);
        break;
      }
    }
  }

  function creative() {
    const preset = file.statements.flatMap((s) => ts.isVariableStatement(s) &&
      (s.declarationList.flags & ts.NodeFlags.Const) !== 0 &&
      s.modifiers?.some((m) => m.kind === ts.SyntaxKind.ExportKeyword) ? [...s.declarationList.declarations] : [])
      .find((d) => ts.isIdentifier(d.name) && d.name.text === "renderStyle");
    if (!preset || imports.length === 0) return;
    const exp = file.statements.find((s) => ts.isExportAssignment(s) && !s.isExportEquals);
    if (!exp || !ts.isExportAssignment(exp)) return;
    const subject = unwrap(exp.expression);
    if (ts.isCallExpression(subject) && api(subject.expression)?.name === "scene") return;
    const decl = ts.isIdentifier(subject) ? symbol(subject)?.valueDeclaration : undefined;
    const init = decl && ts.isVariableDeclaration(decl) && decl.initializer ? unwrap(decl.initializer) : subject;
    const style = staticText(preset.initializer);
    if ((!ts.isArrayLiteralExpression(init) && !checker.isArrayType(checker.getTypeAtLocation(subject)))
        || style === undefined || !["smooth", "toon", "woodblock", "biolume", "papercraft"].includes(style)) {
      report(exp, "creative-scene");
      return;
    }
    // The frozen host read the preset after module evaluation. Inline its literal here because
    // the old default export could precede the declaration of renderStyle.
    const look = style === "smooth" ? helper("toon", exp) + "({ bands: 32, outline: 0 })"
      : helper("styleNamed", exp) + "(" + JSON.stringify(style) + ")";
    set(exp.expression, helper("scene", exp) + "({ things: " + read(exp.expression) + ",\n"
      + "  ground: " + helper("sea", exp) + "({ swell: [] }), style: " + look + ", sun: [-0.45, 0.8, 0.5],\n"
      + '  cameras: { fixed: { kind: "perspective", eye: [4.5, 4.5, 8], target: [0, 0.7, 0], fovY: 0.62 } },\n'
      + "})");
  }
  creative();

  for (const [sym, b] of bindings) {
    const spec = b.specifier;
    if (!spec) continue;
    const refs = references.get(sym) ?? [];
    const reason = manual.get(b.name);
    if (reason) for (const site of refs.length === 0 ? [spec] : refs) report(site, reason);
    const oldCall = rewritten.get(b.name);
    if (oldCall) for (const ref of refs) {
      if (!used.has(ref) && !refused.has(ref)) report(ref, oldCall);
    }
    if (b.name === "cellular2" && b.imported.module.archived)
      for (const ref of refs) if (!used.has(ref)) report(ref, "cellular-result");
  }
  for (const node of nodes) {
    if (ts.isQualifiedName(node) && binding(node.left)?.name === "*") {
      const name = renamed.get(node.right.text);
      if (name) set(node.right, name);
      const reason = manual.get(node.right.text);
      if (reason) report(node, reason);
    }
    const m = access(node);
    if (!m) continue;
    const ns = binding(m.base);
    if (ns?.name === "*") {
      const old = manual.get(m.name) ?? rewritten.get(m.name);
      if (old && !used.has(node) && !refused.has(node)) report(node, old);
      if (m.name === "cellular2" && ns.imported.module.archived && !used.has(node)) report(node, "cellular-result");
    }
    if (m.name === "surfaces" && m.base.kind === ts.SyntaxKind.ThisKeyword && imports.some((i) => i.module.archived))
      report(node, "drawable-forms");
    const owner = api(m.base);
    if (owner?.name === "draw" && ["points", "specks"].includes(m.name) && !used.has(node)) report(node, "point-instancer");
  }

  function target(i: Import, names: readonly string[]) {
    const { file: entry, path, archived } = i.module;
    // One entry: every layer file's names are the entry's, with the same declarations (lane 4.3 checked all six).
    if (path === "loft" || entry === "index.ts" || entry === "lib/wgsl.ts" || entry === "sun.ts" || layers.has(entry.slice(0, -3))) return "loft";
    const abs = resolve(root, "src", entry);
    if (!existsSync(abs) || !abs.startsWith(resolve(root, "src") + "/")) {
      report(i.node, "loft-import-path");
      return path;
    }
    const exports = exportsOf(abs);
    if (i.named && names.every((name) => known(name) &&
        (exports.get(name)?.file === publicNames.get(name)?.file || renamed.has(name) || manual.has(name) || rewritten.has(name)))) return "loft";
    if (output === null) return abs;
    if (!archived && !isAbsolute(path) && output === filename) return path;
    const rel = relative(resolve(output, ".."), abs);
    return rel.startsWith(".") ? rel : "./" + rel;
  }

  function addScreen(name: string, local: string) {
    const text = screen.get(name)!;
    const fn = parseModule(text).fns[0]!.sig.name;
    declarations.push("const " + local + " = " + helper("wgsl", file) + "(" + JSON.stringify(text) + ")." + fn + "!;");
  }

  for (const i of imports) {
    const keep: string[] = [];
    const names: string[] = [];
    let changed = false;
    for (const e of i.named?.elements ?? []) {
      const name = (e.propertyName ?? e.name).text;
      const s = symbol(e.name);
      if (rewritten.has(name) && (s === undefined || (references.get(s) ?? []).every((r) => used.has(r)))) {
        changed = true;
      } else if (screen.has(name) && !e.isTypeOnly && !i.node.importClause?.isTypeOnly) {
        addScreen(name, e.name.text);
        changed = true;
      } else {
        const replacement = renamed.get(name);
        names.push(replacement ?? name);
        keep.push(replacement ? (e.isTypeOnly ? "type " : "") + replacement + " as " + e.name.text : read(e));
        changed ||= replacement !== undefined;
      }
    }
    if (changed && keep.length === 0 && !i.node.importClause?.name) {
      set(i.node, "");
      continue;
    }
    if (changed && i.named) {
      if (keep.length === 0) replace(i.node.importClause!.name!.end, i.named.end, "");
      else set(i.named, "{ " + keep.join(", ") + " }");
    }
    const path = target(i, names);
    if (path !== i.module.path) set(i.node.moduleSpecifier, JSON.stringify(path));
  }
  const injected = [
    ...(helpers.size ? ["import { " + [...helpers].map(([name, local]) => name === local ? name : name + " as " + local).join(", ") + ' } from "loft";'] : []),
    ...declarations,
  ];
  if (injected.length > 0) {
    const last = file.statements.filter(ts.isImportDeclaration).at(-1);
    const work = file.statements.find((s) => !ts.isImportDeclaration(s)
      && !(ts.isExpressionStatement(s) && ts.isStringLiteral(s.expression)));
    // Imports are hoisted. An injected const for an old screen helper must precede its first
    // top-level use even when the original import appeared later in the module.
    const beforeWork = work && (!last || work.pos < last.pos);
    const at = beforeWork ? work.getStart(file) : last?.end ?? code.length;
    replace(at, at, (beforeWork ? "" : "\n") + injected.join("\n") + "\n");
  }
  return result(source(0, code.length));
}

function parseRow(text: string, line: number): { id: string | number; code: string } {
  let row: unknown;
  try { row = JSON.parse(text); }
  catch { throw new Error("JSONL line " + line + " is invalid JSON"); }
  if (typeof row !== "object" || row === null || !("id" in row) || (typeof row.id !== "string" && typeof row.id !== "number")
      || !("code" in row) || typeof row.code !== "string") throw new Error("JSONL line " + line + " needs {id, code}");
  return { id: row.id, code: row.code };
}

async function main(args: string[]) {
  let input: string | undefined, out: string | undefined, json = false, batch = false;
  for (let i = 0; i < args.length; i++) {
    const arg = args[i]!;
    switch (arg) {
      case "--batch": batch = true; break;
      case "--json": json = true; break;
      case "--out":
        out = args[++i];
        if (!out || out.startsWith("--")) throw new Error("--out needs a filename");
        break;
      default:
        if ((arg.startsWith("-") && arg !== "-") || input !== undefined) throw new Error("unexpected argument " + arg);
        input = arg;
    }
  }
  if (!input) throw new Error("usage: bun tools/migrate.ts [--batch] <input.ts | seeds.jsonl | -> [--out file] [--json]");
  const code = await (input === "-" ? Bun.stdin : Bun.file(input)).text();
  if (batch) {
    const next = code.split(/\r?\n/).flatMap((line, i) => {
      if (!line.trim()) return [];
      const { id, code } = parseRow(line, i + 1);
      return [JSON.stringify({ id, ...migrate(code, "scene.ts", null) }) + "\n"];
    }).join("");
    await Bun.write(out ?? Bun.stdout, next);
    return;
  }
  const filename = input === "-" ? "scene.ts" : input;
  const next = migrate(code, filename, out ?? filename);
  if (out) await Bun.write(out, next.code);
  if (json) await Bun.write(Bun.stdout, JSON.stringify(next) + "\n");
  else {
    if (!out) await Bun.write(Bun.stdout, next.code);
    for (const { line, reason } of next.unported) console.error(input + ":" + line + ": " + reason);
  }
}

if (import.meta.main) {
  try { await main(Bun.argv.slice(2)); }
  catch (e) {
    console.error(e instanceof Error ? e.message : String(e));
    process.exitCode = 1;
  }
}
