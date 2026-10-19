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

struct Check { cat: usize, pts: u32, ok: bool, fix: String }

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let o = Command::new("git").arg("-c").arg("core.quotepath=off").arg("-C").arg(dir).args(args).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

/// Lists regular files only. Symlinks are never followed, so a link can't pull in files from outside the folder.
fn walk(root: &Path, dir: &Path, out: &mut Vec<String>, depth: u32) {
    if depth > 40 { return; }
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() { continue; }
        let (p, n) = (e.path(), e.file_name().to_string_lossy().into_owned());
        if ft.is_dir() {
            if !matches!(n.as_str(), ".git" | "target" | "node_modules" | ".venv" | "__pycache__") { walk(root, &p, out, depth + 1); }
        } else if ft.is_file() {
            if let Ok(r) = p.strip_prefix(root) { out.push(r.to_string_lossy().replace('\\', "/")); }
        }
    }
}

fn after(s: &str, pre: &str, n: usize, f: fn(char) -> bool) -> bool {
    s.match_indices(pre).any(|(i, _)| { let t: Vec<char> = s[i + pre.len()..].chars().take(n).collect(); t.len() == n && t.iter().all(|&c| f(c)) })
}

fn is_test_path(p: &str) -> bool {
    let parts: Vec<&str> = p.split('/').collect();
    let name = parts[parts.len() - 1];
    parts[..parts.len() - 1].iter().any(|d| matches!(*d, "test" | "tests" | "__tests__" | "spec" | "specs" | "e2e"))
        || name.starts_with("test_") || name.contains("_test.") || name.contains(".test.") || name.contains(".spec.")
}

fn runs_tests(t: &str) -> bool {
    let t = t.to_lowercase();
    ["cargo test", "cargo nextest", "npm test", "npm run test", "yarn test", "pnpm test", "pytest", "go test", "mvn test", "mvn verify",
     "gradle test", "gradlew test", "dotnet test", "ctest", "make test", "unittest", "rspec", "phpunit"].iter().any(|k| t.contains(k))
}

fn audit(root: &Path) -> (Vec<Check>, bool, bool) {
    let probe = Command::new("git").arg("-C").arg(root).args(["rev-parse", "--is-inside-work-tree"]).output();
    let (is_git, dubious) = match &probe {
        Ok(o) => (o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "true", String::from_utf8_lossy(&o.stderr).contains("dubious ownership")),
        Err(_) => (false, false),
    };
    let files: Vec<String> = if is_git {
        git(root, &["ls-files"]).unwrap_or_default().lines().map(String::from).collect()
    } else { let mut v = vec![]; walk(root, root, &mut v, 0); v };
    let lower: Vec<String> = files.iter().map(|f| f.to_lowercase()).collect();
    // (lowercase path, original path, contents); symlinks skipped, total read budget capped
    let mut texts: Vec<(String, String, String)> = vec![];
    let (mut big, mut budget) = (false, 64_000_000u64);
    for f in &files {
        let path = root.join(f);
        let Ok(m) = fs::symlink_metadata(&path) else { continue };
        if !m.is_file() { continue; }
        if m.len() > 5_000_000 { big = true; }
        let cap = if !f.contains('/') && f.to_lowercase().starts_with("readme") { 1_000_000 } else { 200_000 };
        if m.len() <= cap && m.len() <= budget {
            if let Ok(t) = fs::read_to_string(&path) { budget -= m.len(); texts.push((f.to_lowercase(), f.clone(), t)); }
        }
    }
    let any = |f: &dyn Fn(&str) -> bool| lower.iter().any(|p| f(p));
    let readme = texts.iter().find(|(p, _, _)| p.starts_with("readme") && !p.contains('/')).map(|t| t.2.as_str()).unwrap_or("");
    let rl = readme.to_lowercase();
    let ci_file = |p: &str| p.starts_with(".github/workflows/") || p == ".gitlab-ci.yml";
    let ci = any(&|p| ci_file(p));
    let ci_tests = texts.iter().any(|(p, _, c)| ci_file(p) && runs_tests(c));
    let test_files = lower.iter().filter(|p| is_test_path(p)).count()
        + texts.iter().filter(|(p, _, c)| !is_test_path(p) && ((p.ends_with(".rs") && c.contains("#[test]")) || (p.ends_with(".py") && c.contains("def test_")))).count();
    let log = if is_git { git(root, &["log", "--format=%ct%x09%s", "--", "."]).unwrap_or_default() } else { String::new() };
    let commits: Vec<(i64, &str)> = log.lines().filter_map(|l| l.split_once('\t')).filter_map(|(t, s)| Some((t.parse().ok()?, s))).collect();
    let span = commits.iter().map(|c| c.0).max().unwrap_or(0) - commits.iter().map(|c| c.0).min().unwrap_or(0);
    let lazy = ["update", "fix", "changes", "commit", "wip", "test", "asdf", "."];
    let good = commits.iter().filter(|(_, s)| s.split_whitespace().count() >= 3 && !lazy.contains(&s.trim().to_lowercase().as_str())).count();
    let tags = if is_git { git(root, &["tag"]).unwrap_or_default().lines().count() } else { 0 };
    let junk = any(&|p| p.starts_with("target/") || p.contains("node_modules/") || p.contains("__pycache__/") || p.ends_with(".ds_store") || p.ends_with(".pyc") || p == ".env");
    let (pk, pk2) = (["-----", "BEGIN"].concat(), ["PRIVATE", " KEY"].concat());
    let key_file = texts.iter().find(|(_, _, t)| t.contains(&pk) && t.contains(&pk2)).map(|t| t.1.clone());
    let tok_file = texts.iter().find(|(_, _, t)| after(t, "AKIA", 16, |c| c.is_ascii_uppercase() || c.is_ascii_digit()) || after(t, "ghp_", 30, |c| c.is_ascii_alphanumeric())).map(|t| t.1.clone());
    let kmsg = format!("Private key committed in {}: remove it and rotate it", key_file.clone().unwrap_or_default());
    let tmsg = format!("Token pattern committed in {}: revoke it and purge history", tok_file.clone().unwrap_or_default());
    let mut c = vec![];
    let mut add = |cat: usize, pts: u32, ok: bool, fix: &str| c.push(Check { cat, pts, ok, fix: fix.to_string() });
    add(0, 30, !readme.is_empty(), "Add a README.md");
    add(0, 20, readme.len() >= 1500, "Expand the README: what it does, why it exists, how to run it");
    add(0, 15, readme.lines().filter(|l| l.starts_with('#')).count() >= 3, "Structure the README with at least 3 headings");
    add(0, 15, readme.contains("```"), "Add a runnable usage example in a code block");
    add(0, 10, readme.contains("![") || readme.contains("<img"), "Add a screenshot or GIF to the README");
    add(0, 10, ["install", "usage", "getting started", "cargo run", "npm "].iter().any(|k| rl.contains(k)), "Add an Install/Usage section");
    add(1, 40, test_files > 0, "Add automated tests");
    add(1, 20, test_files >= 3, "Cover more modules: aim for 3+ test files");
    add(1, 40, ci_tests, "Run the test suite in CI (GitHub Actions)");
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
    add(4, 40, key_file.is_none(), &kmsg);
    add(4, 40, tok_file.is_none(), &tmsg);
    add(4, 20, !any(&|p| p == ".env" || p.ends_with("/.env")), ".env is tracked: untrack it");
    add(5, 30, ci, "Add CI (.github/workflows)");
    add(5, 15, any(&|p| p.starts_with("changelog") || p.starts_with("contributing")), "Add CHANGELOG or CONTRIBUTING");
    add(5, 15, rl.contains("shields.io") || rl.contains("badge"), "Add status badges to the README");
    add(5, 15, any(&|p| p.starts_with("assets/") || p.starts_with("docs/") || p.ends_with(".png") || p.ends_with(".gif")), "Add visual assets (assets/ or docs/)");
    add(5, 25, tags > 0, "Tag a release: git tag v0.1.0");
    (c, is_git, dubious)
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
    let esc = name.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;");
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

const USAGE: &str = "usage: questlog [path] [--svg out.svg] [--min-score 0-100] [--no-anim] [--version]";

struct Opts { path: String, svg: Option<String>, min: Option<u32>, anim: bool }

fn parse(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts { path: ".".into(), svg: None, min: None, anim: true };
    let (mut i, mut got_path) = (0, false);
    while i < args.len() {
        match args[i].as_str() {
            "--svg" => { i += 1; o.svg = Some(args.get(i).ok_or("--svg needs a file path")?.clone()); }
            "--min-score" => {
                i += 1;
                let v = args.get(i).ok_or("--min-score needs a number")?;
                let n: u32 = v.parse().map_err(|_| format!("--min-score expects a number from 0 to 100, got '{v}'"))?;
                if n > 100 { return Err(format!("--min-score expects 0 to 100, got {n}")); }
                o.min = Some(n);
            }
            "--no-anim" => o.anim = false,
            a if a.starts_with('-') => return Err(format!("unknown option: {a}")),
            a => { if got_path { return Err("only one path is allowed".into()); } o.path = a.into(); got_path = true; }
        }
        i += 1;
    }
    Ok(o)
}

fn main() {
    enable_vt();
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") { println!("questlog {} - repo audit as a pixel-RPG sheet\n\n{USAGE}", env!("CARGO_PKG_VERSION")); return; }
    if args.iter().any(|a| a == "--version" || a == "-V") { println!("questlog {}", env!("CARGO_PKG_VERSION")); return; }
    let o = match parse(&args) { Ok(o) => o, Err(e) => { eprintln!("error: {e}\n{USAGE}"); exit(2); } };
    let root = Path::new(&o.path);
    if !root.is_dir() { eprintln!("error: not a directory: {}", o.path); exit(2); }
    if Command::new("git").arg("--version").output().is_err() { eprintln!("error: questlog needs git on PATH: https://git-scm.com/downloads"); exit(2); }
    let (checks, is_git, dubious) = audit(root);
    let mut sc = [0u32; 6];
    for c in checks.iter().filter(|c| c.ok) { sc[c.cat] += c.pts; }
    let overall = sc.iter().sum::<u32>() / 6;
    let raw = fs::canonicalize(root).ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).unwrap_or_else(|| o.path.clone());
    let clean: String = raw.chars().filter(|c| !c.is_control()).collect();
    let name = if clean.is_empty() { "repo".to_string() } else if clean.chars().count() > 24 { format!("{}…", clean.chars().take(23).collect::<String>()) } else { clean };
    let narrow = env::var("COLUMNS").ok().and_then(|c| c.parse::<u32>().ok()).map_or(false, |c| c < 62);
    let on = std::io::stdout().is_terminal() && !narrow && env::var_os("NO_COLOR").is_none() && env::var("TERM").map_or(true, |t| t != "dumb");
    println!();
    for l in banner(on) { println!("{l}"); }
    println!();
    if on && o.anim {
        let mut out = std::io::stdout();
        for f in 0..=24 {
            let k = 1.0 - (1.0 - f as f32 / 24.0).powi(3);
            if f > 0 { print!("\x1b[12A"); }
            for l in &sheet(&name, &sc, k, overall, on, f as f32 / 12.0) { println!("{l}\x1b[K"); }
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
    if dubious { println!("\n  note: git refuses this folder (owned by another user). Run: git config --global --add safe.directory <path>"); }
    else if !is_git { println!("\n  note: not a git repo, so history can't be scored."); }
    println!("\n  clear every quest: {overall} -> {potential}/100");
    if let Some(p) = &o.svg {
        match fs::write(p, svg(&name, &sc, overall)) { Ok(_) => println!("  card written: {p}"), Err(e) => { eprintln!("error: cannot write {p}: {e}"); exit(2); } }
    }
    if let Some(m) = o.min { if overall < m { eprintln!("score {overall} is below required {m}"); exit(1); } }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sv(a: &[&str]) -> Vec<String> { a.iter().map(|s| s.to_string()).collect() }
    fn temp(tag: &str) -> std::path::PathBuf {
        let d = env::temp_dir().join(format!("questlog-{tag}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&d).unwrap();
        d
    }
    #[test] fn ranks_map_to_tiers() { assert_eq!(tier(0), 0); assert_eq!(tier(59), 2); assert_eq!(tier(60), 3); assert_eq!(tier(100), 5); }
    #[test] fn token_scanner_needs_full_length() {
        let full = ["k=AKI", "AABCDEFGHIJKLMNOP"].concat();
        assert!(after(&full, "AKIA", 16, |c| c.is_ascii_uppercase()));
        assert!(!after("AKIA short", "AKIA", 16, |c| c.is_ascii_uppercase()));
    }
    #[test] fn check_points_sum_to_100_per_category() {
        let (c, _, _) = audit(Path::new("."));
        for i in 0..6 { assert_eq!(c.iter().filter(|x| x.cat == i).map(|x| x.pts).sum::<u32>(), 100, "{}", CATS[i]); }
    }
    #[test] fn test_path_detection_avoids_lookalikes() {
        for p in ["tests/a.rs", "src/__tests__/x.js", "app.test.js", "pkg/test_util.py", "x/foo_test.go"] { assert!(is_test_path(p), "{p}"); }
        for p in ["docs/latest.md", "src/inspect.rs", "contest.py", "specification.md", "src/attest.rs"] { assert!(!is_test_path(p), "{p}"); }
    }
    #[test] fn ci_test_detection_is_specific() {
        assert!(runs_tests("run: cargo test --all"));
        assert!(runs_tests("run: npm test"));
        assert!(!runs_tests("uses: actions/checkout@latest\nrun: echo hello"));
    }
    #[test] fn args_are_strictly_validated() {
        assert!(parse(&sv(&["--min-score", "abc"])).is_err());
        assert!(parse(&sv(&["--min-score", "101"])).is_err());
        assert!(parse(&sv(&["--min-score"])).is_err());
        assert!(parse(&sv(&["--svg"])).is_err());
        assert!(parse(&sv(&["--bogus"])).is_err());
        assert!(parse(&sv(&["a", "b"])).is_err());
        let o = parse(&sv(&["../x", "--svg", "c.svg", "--min-score", "70", "--no-anim"])).unwrap();
        assert_eq!((o.path.as_str(), o.svg.as_deref(), o.min, o.anim), ("../x", Some("c.svg"), Some(70), false));
    }
    #[test] fn committed_secret_is_detected_and_named() {
        if Command::new("git").arg("--version").output().is_err() { return; }
        let d = temp("leak");
        let run = |a: &[&str]| Command::new("git").arg("-C").arg(&d).args(a).output().unwrap();
        run(&["init", "-q"]);
        fs::write(d.join("leak.py"), format!("k='{}{}'", "AKIA", "ABCDEFGHIJKLMNOP")).unwrap();
        run(&["add", "."]);
        run(&["-c", "user.name=t", "-c", "user.email=t@example.invalid", "commit", "-qm", "add leaked token file"]);
        let (c, is_git, _) = audit(&d);
        assert!(is_git);
        assert!(c.iter().any(|x| x.cat == 4 && !x.ok && x.fix.contains("leak.py")));
        fs::remove_dir_all(&d).ok();
    }
    #[cfg(unix)]
    #[test] fn symlinks_are_never_followed() {
        let d = temp("link");
        fs::write(d.join("real.txt"), "x").unwrap();
        std::os::unix::fs::symlink("/etc", d.join("etc-link")).unwrap();
        std::os::unix::fs::symlink(&d, d.join("loop")).unwrap();
        let mut v = vec![];
        walk(&d, &d, &mut v, 0);
        assert_eq!(v, vec!["real.txt".to_string()]);
        fs::remove_dir_all(&d).ok();
    }
}
