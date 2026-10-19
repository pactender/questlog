//! questlog: audits a repo the way a reviewer skims it, then renders the result as a pixel-RPG
//! character sheet plus a fix list ranked by score gain. Offline, std-only, reads tracked files.
use std::{env, fs, io::{IsTerminal, Write}, path::Path, process::{exit, Command}, thread::sleep, time::Duration};

const CATS: [&str; 6] = ["DOCS", "TESTS", "HYGIENE", "HISTORY", "SECURITY", "POLISH"];
const RANKS: [(u32, &str); 6] = [(0, "WANDERER"), (20, "APPRENTICE"), (40, "JOURNEYMAN"), (60, "ADEPT"), (75, "MASTER"), (90, "LEGEND")];
// (armor, cape, helmet) per rank
const PAL: [([u8; 3], [u8; 3], [u8; 3]); 6] = [
    ([130, 120, 110], [110, 80, 60], [150, 140, 130]), ([90, 170, 110], [60, 120, 80], [120, 190, 130]),
    ([80, 140, 220], [50, 90, 170], [130, 180, 240]), ([160, 100, 220], [110, 60, 170], [190, 140, 240]),
    ([240, 190, 70], [200, 60, 60], [255, 220, 110]), ([120, 235, 245], [90, 130, 230], [235, 250, 255]),
];
const SPRITE: [&str; 12] = [
    "....HHHH....", "...HHGHHH...", "...SSSSSS...", "...SESSES...", "...SSSSSS...", "..CCCCCCCC..",
    ".CCAAAAAACC.", ".CCAAAAAACC.", "..SAAAAAAS..", "...AA..AA...", "...AA..AA...", "...BB..BB...",
];

struct Check { cat: usize, pts: u32, ok: bool, fix: &'static str }

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let o = Command::new("git").arg("-c").arg("core.quotepath=off").arg("-C").arg(dir).args(args).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let (p, n) = (e.path(), e.file_name().to_string_lossy().into_owned());
        if p.is_dir() {
            if !matches!(n.as_str(), ".git" | "target" | "node_modules" | ".venv" | "__pycache__") { walk(root, &p, out); }
        } else if let Ok(r) = p.strip_prefix(root) { out.push(r.to_string_lossy().replace('\\', "/")); }
    }
}

fn after(s: &str, pre: &str, n: usize, f: fn(char) -> bool) -> bool {
    s.match_indices(pre).any(|(i, _)| { let t: Vec<char> = s[i + pre.len()..].chars().take(n).collect(); t.len() == n && t.iter().all(|&c| f(c)) })
}

fn audit(root: &Path) -> (Vec<Check>, bool) {
    let is_git = git(root, &["rev-parse", "--is-inside-work-tree"]).map_or(false, |s| s.trim() == "true");
    let files: Vec<String> = if is_git {
        git(root, &["ls-files"]).unwrap_or_default().lines().map(String::from).collect()
    } else { let mut v = vec![]; walk(root, root, &mut v); v };
    let lower: Vec<String> = files.iter().map(|f| f.to_lowercase()).collect();
    let mut texts: Vec<(String, String)> = vec![];
    let mut big = false;
    for f in &files {
        let Ok(m) = fs::metadata(root.join(f)) else { continue };
        if m.len() > 5_000_000 { big = true; }
        if m.len() <= 200_000 { if let Ok(t) = fs::read_to_string(root.join(f)) { texts.push((f.to_lowercase(), t)); } }
    }
    let any = |f: &dyn Fn(&str) -> bool| lower.iter().any(|p| f(p));
    let readme = texts.iter().find(|(p, _)| p.starts_with("readme") && !p.contains('/')).map(|(_, t)| t.as_str()).unwrap_or("");
    let rl = readme.to_lowercase();
    let wf = |t: &str| texts.iter().any(|(p, c)| p.starts_with(".github/workflows/") && c.to_lowercase().contains(t));
    let ci = any(&|p| p.starts_with(".github/workflows/")) || any(&|p| p == ".gitlab-ci.yml");
    let test_files = texts.iter().filter(|(p, c)| p.contains("test") || p.contains("spec") || c.contains("#[test]") || c.contains("def test_")).count();
    let log = if is_git { git(root, &["log", "--format=%ct%x09%s"]).unwrap_or_default() } else { String::new() };
    let commits: Vec<(i64, &str)> = log.lines().filter_map(|l| l.split_once('\t')).filter_map(|(t, s)| Some((t.parse().ok()?, s))).collect();
    let span = commits.iter().map(|c| c.0).max().unwrap_or(0) - commits.iter().map(|c| c.0).min().unwrap_or(0);
    let lazy = ["update", "fix", "changes", "commit", "wip", "test", "asdf", "."];
    let good = commits.iter().filter(|(_, s)| s.split_whitespace().count() >= 3 && !lazy.contains(&s.trim().to_lowercase().as_str())).count();
    let tags = if is_git { git(root, &["tag"]).unwrap_or_default().lines().count() } else { 0 };
    let junk = any(&|p| p.starts_with("target/") || p.contains("node_modules/") || p.contains("__pycache__/") || p.ends_with(".ds_store") || p.ends_with(".pyc") || p == ".env");
    let pk = ["-----", "BEGIN"].concat();
    let key = texts.iter().any(|(_, t)| t.contains(&pk) && t.contains(&["PRIVATE", " KEY"].concat()));
    let tok = texts.iter().any(|(_, t)| after(t, "AKIA", 16, |c| c.is_ascii_uppercase() || c.is_ascii_digit()) || after(t, "ghp_", 30, |c| c.is_ascii_alphanumeric()));
    let mut c = vec![];
    let mut add = |cat, pts, ok, fix| c.push(Check { cat, pts, ok, fix });
    add(0, 30, !readme.is_empty(), "Add a README.md");
    add(0, 20, readme.len() >= 1500, "Expand the README: what it does, why it exists, how to run it");
    add(0, 15, readme.lines().filter(|l| l.starts_with('#')).count() >= 3, "Structure the README with at least 3 headings");
    add(0, 15, readme.contains("```"), "Add a runnable usage example in a code block");
    add(0, 10, readme.contains("![") || readme.contains("<img"), "Add a screenshot or GIF to the README");
    add(0, 10, ["install", "usage", "getting started", "cargo run", "npm "].iter().any(|k| rl.contains(k)), "Add an Install/Usage section");
    add(1, 40, test_files > 0, "Add automated tests");
    add(1, 20, test_files >= 3, "Cover more modules: aim for 3+ test files");
    add(1, 40, wf("test"), "Run the test suite in CI (GitHub Actions)");
    add(2, 25, any(&|p| p.starts_with("license") || p.starts_with("copying")), "Add a LICENSE file");
    add(2, 20, any(&|p| p == ".gitignore"), "Add a .gitignore");
    add(2, 15, any(&|p| ["cargo.toml", "package.json", "pyproject.toml", "go.mod", "requirements.txt", "pom.xml", "build.gradle"].contains(&p)), "Add a dependency manifest");
    add(2, 25, !junk, "Untrack build output / .env / caches (git rm -r --cached)");
    add(2, 15, !big, "Remove files over 5 MB from the repo");
    add(3, 20, is_git, "Put the project under git");
    add(3, 20, commits.len() >= 10, "Build real history: 10+ commits");
    add(3, 10, commits.len() >= 30, "Keep committing in small steps: 30+ commits");
    add(3, 20, span >= 14 * 86400, "History should span 2+ weeks of work, not one dump");
    add(3, 30, !commits.is_empty() && good * 10 >= commits.len() * 7, "Write descriptive commit messages (3+ words, not 'update'/'fix')");
    add(4, 40, !key, "A private key is committed: remove it and rotate it");
    add(4, 40, !tok, "A cloud/API token pattern is committed: revoke it and purge history");
    add(4, 20, !any(&|p| p == ".env" || p.ends_with("/.env")), ".env is tracked: untrack it");
    add(5, 30, ci, "Add CI (.github/workflows)");
    add(5, 15, any(&|p| p.starts_with("changelog") || p.starts_with("contributing")), "Add CHANGELOG or CONTRIBUTING");
    add(5, 15, rl.contains("shields.io") || rl.contains("badge"), "Add status badges to the README");
    add(5, 15, any(&|p| p.starts_with("assets/") || p.starts_with("docs/") || p.ends_with(".png") || p.ends_with(".gif")), "Add visual assets (assets/ or docs/)");
    add(5, 25, tags > 0, "Tag a release: git tag v0.1.0");
    (c, is_git)
}

#[cfg(windows)]
fn enable_vt() {
    use std::ffi::c_void;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetStdHandle(n: u32) -> *mut c_void;
        fn GetConsoleMode(h: *mut c_void, m: *mut u32) -> i32;
        fn SetConsoleMode(h: *mut c_void, m: u32) -> i32;
        fn SetConsoleOutputCP(cp: u32) -> i32;
    }
    unsafe {
        SetConsoleOutputCP(65001);
        let h = GetStdHandle(-11i32 as u32);
        let mut m = 0;
        if GetConsoleMode(h, &mut m) != 0 { SetConsoleMode(h, m | 0x0004); }
    }
}
#[cfg(not(windows))]
fn enable_vt() {}

fn banner(on: bool) -> Vec<String> {
    if !on { return vec![format!("QUESTLOG {}", env!("CARGO_PKG_VERSION"))]; }
    const G: [(char, [&str; 5]); 8] = [
        ('Q', ["01110", "10001", "10101", "10010", "01101"]), ('U', ["10001", "10001", "10001", "10001", "01110"]),
        ('E', ["11111", "10000", "11110", "10000", "11111"]), ('S', ["01111", "10000", "01110", "00001", "11110"]),
        ('T', ["11111", "00100", "00100", "00100", "00100"]), ('L', ["10000", "10000", "10000", "10000", "11111"]),
        ('O', ["01110", "10001", "10001", "10001", "01110"]), ('G', ["01111", "10000", "10011", "10001", "01111"]),
    ];
    let mut px = vec![vec![None::<[u8; 3]>; 47]; 6];
    for (i, ch) in "QUESTLOG".chars().enumerate() {
        let g = G.iter().find(|g| g.0 == ch).unwrap().1;
        for (y, row) in g.iter().enumerate() { for (x, b) in row.bytes().enumerate() {
            if b == b'1' { px[y][i * 6 + x] = Some([255, (240 - y * 22) as u8, (150 - y * 20) as u8]); }
        } }
    }
    let mut out = vec![];
    for r in (0..6).step_by(2) {
        let mut l = String::from(" ");
        for x in 0..47 {
            l += &match (px[r][x], px[r + 1][x]) {
                (None, None) => " ".into(),
                (Some(t), None) => format!("\x1b[38;2;{};{};{}m▀\x1b[0m", t[0], t[1], t[2]),
                (None, Some(b)) => format!("\x1b[38;2;{};{};{}m▄\x1b[0m", b[0], b[1], b[2]),
                (Some(t), Some(b)) => format!("\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m▀\x1b[0m", t[0], t[1], t[2], b[0], b[1], b[2]),
            };
        }
        out.push(l);
    }
    out.push(rgb([120, 114, 160], &format!(" audit your repo like a recruiter would  ·  v{}", env!("CARGO_PKG_VERSION")), true));
    out
}

fn rgb(c: [u8; 3], s: &str, on: bool) -> String { if on { format!("\x1b[38;2;{};{};{}m{}\x1b[0m", c[0], c[1], c[2], s) } else { s.into() } }
fn tier(overall: u32) -> usize { RANKS.iter().rposition(|r| overall >= r.0).unwrap_or(0) }

fn sprite(t: usize) -> Vec<Vec<Option<[u8; 3]>>> {
    let (a, c, h) = PAL[t];
    SPRITE.iter().map(|r| r.chars().map(|ch| match ch {
        'H' => Some(h), 'G' => Some(if t >= 3 { [255, 230, 120] } else { h }), 'S' => Some([232, 190, 150]), 'E' => Some([20, 20, 30]),
        'C' => Some(c), 'A' => Some(a), 'B' => Some([70, 50, 40]), _ => None,
    }).collect()).collect()
}

fn bar(v: f32, w: usize, on: bool) -> String {
    let n = (v / 100.0 * w as f32).round() as usize;
    let col = if v < 40.0 { [220, 70, 70] } else if v < 70.0 { [240, 190, 80] } else { [90, 210, 130] };
    format!("{}{}", rgb(col, &"█".repeat(n), on), rgb([60, 60, 85], &"░".repeat(w - n), on))
}

fn sheet(name: &str, sc: &[u32; 6], shown: f32, overall: u32, on: bool, t: f32) -> Vec<String> {
    let tr = tier(overall);
    let k = shown;
    let mut right = vec![
        rgb([255, 214, 110], &format!("{}{}", if on { "\x1b[1m" } else { "" }, name.to_uppercase()), on), format!("LV {}  {}", (overall + 9) / 10, RANKS[tr].1), String::new(),
    ];
    for i in 0..6 { right.push(format!("{:<9}{} {:>3}", CATS[i], bar(sc[i] as f32 * k, 20, on), (sc[i] as f32 * k) as u32)); }
    right.extend([String::new(), format!("OVERALL  {}/100", (overall as f32 * k) as u32), String::new()]);
    let bg = [24, 22, 40];
    sprite(tr).iter().enumerate().map(|(y, row)| {
        let px: String = row.iter().enumerate().map(|(x, p)| {
            let twinkle = tr >= 4 && p.is_none() && ((x * 7 + y * 3) as f32 + t * 6.0) as usize % 17 == 0;
            let c = p.unwrap_or(if twinkle { [90, 90, 140] } else { bg });
            if on { format!("\x1b[48;2;{};{};{}m  \x1b[0m", c[0], c[1], c[2]) } else { "  ".into() }
        }).collect();
        if on { format!("{}   {}", px, right[y]) } else { right[y].clone() }
    }).collect()
}

fn svg(name: &str, sc: &[u32; 6], overall: u32) -> String {
    let tr = tier(overall);
    let hex = |c: [u8; 3]| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
    let esc = name.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut s = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 560 300\" font-family=\"monospace\" shape-rendering=\"crispEdges\"><defs><radialGradient id=\"g\"><stop offset=\"0\" stop-color=\"{}\" stop-opacity=\".5\"/><stop offset=\"1\" stop-color=\"{}\" stop-opacity=\"0\"/></radialGradient></defs><rect width=\"560\" height=\"300\" rx=\"10\" fill=\"#0f0e1a\"/><rect x=\"6\" y=\"6\" width=\"548\" height=\"288\" rx=\"6\" fill=\"none\" stroke=\"{}\" stroke-width=\"2\"/>", hex(PAL[tr].0), hex(PAL[tr].0), hex(PAL[tr].0));
    s += "<circle cx=\"90\" cy=\"150\" r=\"90\" fill=\"url(#g)\"/>";
    for (y, row) in sprite(tr).iter().enumerate() { for (x, p) in row.iter().enumerate() {
        if let Some(c) = p { s += &format!("<rect x=\"{}\" y=\"{}\" width=\"10\" height=\"10\" fill=\"{}\"/>", 30 + x * 10, 90 + y * 10, hex(*c)); }
    } }
    s += &format!("<text x=\"180\" y=\"54\" font-size=\"24\" font-weight=\"bold\" fill=\"#ffd66e\">{}</text><text x=\"180\" y=\"80\" font-size=\"14\" fill=\"#b9b4e0\">LV {}  {}</text>", esc, (overall + 9) / 10, RANKS[tr].1);
    for i in 0..6 {
        let y = 112 + i * 27;
        let col = if sc[i] < 40 { "#dc4646" } else if sc[i] < 70 { "#f0be50" } else { "#5ad282" };
        s += &format!("<text x=\"180\" y=\"{}\" font-size=\"13\" fill=\"#b9b4e0\">{}</text><rect x=\"280\" y=\"{}\" width=\"200\" height=\"12\" fill=\"#3c3c55\"/><rect x=\"280\" y=\"{}\" width=\"{}\" height=\"12\" fill=\"{}\"/><text x=\"492\" y=\"{}\" font-size=\"13\" fill=\"#fff\">{}</text>", y + 11, CATS[i], y, y, sc[i] * 2, col, y + 11, sc[i]);
    }
    s + &format!("<text x=\"180\" y=\"284\" font-size=\"12\" fill=\"#6d6a90\">questlog  overall {}/100</text></svg>", overall)
}

fn main() {
    enable_vt();
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") { println!("questlog {}", env!("CARGO_PKG_VERSION")); return; }
    let opt = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    if args.iter().any(|a| a == "-h" || a == "--help") { println!("questlog {} - repo audit as a pixel-RPG sheet\n\nusage: questlog [path] [--svg out.svg] [--min-score N] [--no-anim] [--version]", env!("CARGO_PKG_VERSION")); return; }
    let path = args.iter().enumerate().find(|(i, a)| !a.starts_with("--") && (*i == 0 || !args[i - 1].starts_with("--") || args[i - 1] == "--no-anim")).map_or(".".to_string(), |(_, a)| a.clone());
    let root = Path::new(&path);
    if !root.is_dir() { eprintln!("not a directory: {path}"); exit(2); }
    let (checks, is_git) = audit(root);
    let mut sc = [0u32; 6];
    for c in checks.iter().filter(|c| c.ok) { sc[c.cat] += c.pts; }
    let overall = sc.iter().sum::<u32>() / 6;
    let name = fs::canonicalize(root).ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).unwrap_or(path.clone());
    let tty = std::io::stdout().is_terminal();
    let on = tty;
    if git(root, &["--version"]).is_none() && Command::new("git").arg("--version").output().is_err() { eprintln!("questlog needs git on PATH: https://git-scm.com/downloads"); exit(2); }
    println!();
    for l in banner(on) { println!("{l}"); }
    println!();
    if on && !args.iter().any(|a| a == "--no-anim") {
        let mut out = std::io::stdout();
        for f in 0..=24 {
            let k = 1.0 - (1.0 - f as f32 / 24.0).powi(3);
            let lines = sheet(&name, &sc, k, overall, on, f as f32 / 12.0);
            if f > 0 { print!("\x1b[12A"); }
            for l in &lines { println!("{l}\x1b[K"); }
            out.flush().ok();
            sleep(Duration::from_millis(28));
        }
    } else {
        for l in sheet(&name, &sc, 1.0, overall, on, 0.0) { println!("{l}"); }
    }
    let mut q: Vec<&Check> = checks.iter().filter(|c| !c.ok).collect();
    q.sort_by_key(|c| std::cmp::Reverse(c.pts));
    let potential = (sc.iter().sum::<u32>() + q.iter().map(|c| c.pts).sum::<u32>()) / 6;
    println!("\n{}", rgb([255, 214, 110], "QUESTS  (ranked by score gain)", on));
    if q.is_empty() { println!("  none left. Ship it."); }
    for c in q.iter().take(7) { println!("  +{:<2} XP  [{}] {}", (c.pts as f32 / 6.0).round().max(1.0) as u32, CATS[c.cat], c.fix); }
    if !is_git { println!("\n  note: not a git repo, so history can't be scored."); }
    println!("\n  clear every quest: {overall} -> {potential}/100");
    if let Some(p) = opt("--svg") { fs::write(&p, svg(&name, &sc, overall)).expect("write svg"); println!("  card written: {p}"); }
    if let Some(m) = opt("--min-score").and_then(|m| m.parse::<u32>().ok()) { if overall < m { eprintln!("score {overall} < required {m}"); exit(1); } }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn ranks_map_to_tiers() { assert_eq!(tier(0), 0); assert_eq!(tier(59), 2); assert_eq!(tier(60), 3); assert_eq!(tier(100), 5); }
    #[test] fn token_scanner_needs_full_length() {
        let full = ["k=AKI", "AABCDEFGHIJKLMNOP"].concat();
        assert!(after(&full, "AKIA", 16, |c| c.is_ascii_uppercase()));
        assert!(!after("AKIA short", "AKIA", 16, |c| c.is_ascii_uppercase()));
    }
    #[test] fn check_points_sum_to_100_per_category() {
        let (c, _) = audit(Path::new("."));
        for i in 0..6 { assert_eq!(c.iter().filter(|x| x.cat == i).map(|x| x.pts).sum::<u32>(), 100, "{}", CATS[i]); }
    }
}
