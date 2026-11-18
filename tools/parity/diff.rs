//! parity-diff: one frame against its reference under the picture-parity gate
//! (docs/proposals/2026-11-19_lean_rust_loft.md §2.2a, §2.3). Moves into `loft-draw diff` in lane 3.3.
//!
//!   parity-diff <reference.png> <candidate.png> [--rule picture|cross] [--envelope N]
//!
//! Prints one JSON line and exits 0 (pass), 1 (fail) or 2 (usage, unreadable PNG).
//!
//! A pixel is *over* when any 8-bit channel differs by more than the rule's tolerance. Picture rule: every over pixel
//! lies within 1 px of a reference edge (rule 1 and the second half of rule 3), at most 0.01% of the frame is over
//! (rule 2), and no 4-connected cluster of over pixels exceeds 4 (rule 3). Cross-executor rule (§2.3, Dawn
//! references): tolerance 8/255, fewer than 0.01% over, clusters of at most 4, no edge condition.
//! `--envelope N` widens the tolerance by the measured run-to-run envelope (§2.3 precondition), in 8-bit codes.
//!
//! Edges: the plan's rule 3 names reference *depth* discontinuities. Today's executor (`loft-view`) writes no depth, so
//! the edge map is taken from the reference picture: a pixel is an edge when an RGB channel jumps by more than
//! `EDGE_JUMP` to a 4-neighbour. The report names this (`"edges":"picture-jump-32"`); a depth source joins when
//! `loft-draw` writes one.

use std::process::ExitCode;

const EDGE_JUMP: u8 = 32;

enum Rule {
    Picture,
    Cross,
}

impl Rule {
    fn parse(s: &str) -> Result<Rule, String> {
        match s {
            "picture" => Ok(Rule::Picture),
            "cross" => Ok(Rule::Cross),
            other => Err(format!("--rule: expected picture or cross, got {other}")),
        }
    }
    fn name(&self) -> &'static str {
        match self {
            Rule::Picture => "picture",
            Rule::Cross => "cross",
        }
    }
    /// (tolerance in 8-bit codes, whether over pixels must sit by an edge, the over-pixel limit for `n` pixels).
    fn terms(&self, envelope: u8, n: usize) -> (u8, bool, usize) {
        match self {
            // At most 0.01%: 26 of 262,144 at 512².
            Rule::Picture => (1 + envelope, true, n / 10_000),
            // Fewer than 0.01%.
            Rule::Cross => (8 + envelope, false, (n - 1) / 10_000),
        }
    }
}

struct Image {
    w: usize,
    h: usize,
    rgba: Vec<u8>,
}

#[derive(Debug, PartialEq)]
struct Report {
    over: usize,
    limit: usize,
    max_cluster: usize,
    off_edge: usize,
    max_diff: u8,
    tol: u8,
}

impl Report {
    fn pass(&self) -> bool {
        self.over <= self.limit && self.max_cluster <= 4 && self.off_edge == 0
    }
}

fn read(path: &str) -> Result<Image, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| format!("{path}: {e}"))?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or_else(|| format!("{path}: image too large"))?];
    let info = reader.next_frame(&mut buf).map_err(|e| format!("{path}: {e}"))?;
    let (w, h) = (info.width as usize, info.height as usize);
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..w * h * 4].to_vec(),
        png::ColorType::Rgb => buf[..w * h * 3].chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::Grayscale | png::ColorType::GrayscaleAlpha | png::ColorType::Indexed => {
            return Err(format!("{path}: {:?} PNG; the gate compares RGB(A) frames as the executors write them", info.color_type))
        }
    };
    Ok(Image { w, h, rgba })
}

fn px(img: &Image, i: usize) -> &[u8] {
    &img.rgba[i * 4..i * 4 + 4]
}

fn compare(r: &Image, c: &Image, rule: &Rule, envelope: u8) -> Report {
    let n = r.w * r.h;
    let (tol, by_edge, limit) = rule.terms(envelope, n);
    let jump = |a: &[u8], b: &[u8]| (0..3).any(|k| a[k].abs_diff(b[k]) > EDGE_JUMP);
    let neighbours = |i: usize| {
        let (x, y) = (i % r.w, i / r.w);
        [(x > 0).then(|| i - 1), (x + 1 < r.w).then(|| i + 1), (y > 0).then(|| i - r.w), (y + 1 < r.h).then(|| i + r.w)]
    };
    let edge: Vec<bool> = (0..n).map(|i| neighbours(i).into_iter().flatten().any(|j| jump(px(r, i), px(r, j)))).collect();
    let mut max_diff = 0u8;
    let over: Vec<bool> = (0..n)
        .map(|i| {
            let d = (0..4).fold(0, |m, k| m.max(px(r, i)[k].abs_diff(px(c, i)[k])));
            max_diff = max_diff.max(d);
            d > tol
        })
        .collect();
    let near_edge = |i: usize| {
        let (x, y) = ((i % r.w) as isize, (i / r.w) as isize);
        (-1..=1isize).any(|dy| {
            (-1..=1isize).any(|dx| {
                let (u, v) = (x + dx, y + dy);
                u >= 0 && v >= 0 && (u as usize) < r.w && (v as usize) < r.h && edge[v as usize * r.w + u as usize]
            })
        })
    };
    let off_edge = if by_edge { (0..n).filter(|&i| over[i] && !near_edge(i)).count() } else { 0 };
    let (mut seen, mut max_cluster, mut stack) = (vec![false; n], 0, Vec::new());
    for s in (0..n).filter(|&i| over[i]) {
        if seen[s] {
            continue;
        }
        seen[s] = true;
        stack.push(s);
        let mut size = 0;
        while let Some(i) = stack.pop() {
            size += 1;
            for j in neighbours(i).into_iter().flatten() {
                if over[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        max_cluster = max_cluster.max(size);
    }
    Report { over: over.iter().filter(|&&o| o).count(), limit, max_cluster, off_edge, max_diff, tol }
}

fn run(args: &[String]) -> Result<bool, String> {
    let (mut paths, mut rule, mut envelope) = (Vec::new(), Rule::Picture, 0u8);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--rule" => rule = Rule::parse(it.next().ok_or("--rule needs picture or cross")?)?,
            "--envelope" => {
                let v = it.next().ok_or("--envelope needs a count of 8-bit codes")?;
                envelope = v.parse().map_err(|_| format!("--envelope: expected 0..255, got {v}"))?;
            }
            p if !p.starts_with("--") => paths.push(p.to_string()),
            other => return Err(format!("unknown flag {other}")),
        }
    }
    let [reference, candidate] = paths.as_slice() else {
        return Err("usage: parity-diff <reference.png> <candidate.png> [--rule picture|cross] [--envelope N]".into());
    };
    let (r, c) = (read(reference)?, read(candidate)?);
    let rule_name = rule.name();
    if (r.w, r.h) != (c.w, c.h) {
        println!(r#"{{"pass":false,"rule":"{rule_name}","fault":"size","reference":[{},{}],"candidate":[{},{}]}}"#, r.w, r.h, c.w, c.h);
        return Ok(false);
    }
    let rep = compare(&r, &c, &rule, envelope);
    println!(
        r#"{{"pass":{},"rule":"{rule_name}","edges":"picture-jump-{EDGE_JUMP}","tol":{},"over":{},"limit":{},"maxCluster":{},"offEdge":{},"maxDiff":{},"size":[{},{}]}}"#,
        rep.pass(), rep.tol, rep.over, rep.limit, rep.max_cluster, rep.off_edge, rep.max_diff, r.w, r.h
    );
    Ok(rep.pass())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(msg) => {
            eprintln!("parity-diff: {msg}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 512² frame: dark left of column `edge`, light right of it, with a soft vertical gradient so it is not flat.
    fn step(edge: usize) -> Image {
        let (w, h) = (512, 512);
        let rgba = (0..w * h)
            .flat_map(|i| {
                let (x, y) = (i % w, i / w);
                let v = if x < edge { 40 } else { 200 } + (y / 64) as u8;
                [v, v / 2, 255 - v, 255]
            })
            .collect();
        Image { w, h, rgba }
    }

    fn check(c: &Image) -> Report {
        compare(&step(256), c, &Rule::Picture, 0)
    }

    #[test]
    fn identical_and_one_code_pass() {
        assert!(check(&step(256)).pass());
        let mut c = step(256);
        c.rgba.iter_mut().step_by(3).for_each(|v| *v = v.saturating_add(1));
        assert!(check(&c).pass());
    }

    #[test]
    fn a_planted_two_pixel_shifted_edge_fails() {
        // The silhouette moved 2 px right over a 64-row stretch: a 2-wide band of 128 over pixels, all by the edge.
        let mut c = step(256);
        let r = step(258);
        for y in 200..264 {
            let row = y * 512 * 4;
            c.rgba[row..row + 512 * 4].copy_from_slice(&r.rgba[row..row + 512 * 4]);
        }
        let rep = check(&c);
        assert_eq!((rep.over, rep.max_cluster, rep.off_edge), (128, 128, 0));
        assert!(!rep.pass());
    }

    #[test]
    fn scattered_edge_flips_pass_but_an_off_edge_pixel_fails() {
        let mut c = step(256);
        for y in [10, 100, 300] {
            c.rgba[(y * 512 + 256) * 4] = 40; // a flipped pixel on the silhouette
        }
        assert!(check(&c).pass());
        c.rgba[(50 * 512 + 50) * 4] += 5; // 5/255 in the flat interior
        let rep = check(&c);
        assert_eq!(rep.off_edge, 1);
        assert!(!rep.pass());
    }
}
