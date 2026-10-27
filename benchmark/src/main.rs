// Compares regex-anre with the `regex` crate on the corpora in `../examples`.
//
// Run with: `cargo run --release [-- <case-name-filter>]`

use std::{
    fs,
    hint::black_box,
    path::Path,
    time::{Duration, Instant},
};

struct Case {
    name: &'static str,
    corpus: &'static str,
    anre: &'static str,
    // The `regex` crate is Unicode-aware by default, so some classes are made ASCII-only to match regex-anre.
    regex: &'static str,
    captures: bool,
}

const CASES: &[Case] = &[
    Case { name: "literal", corpus: "english.txt", anre: "pattern", regex: "pattern", captures: false },
    Case { name: "literal-rare", corpus: "english.txt", anre: "university", regex: "university", captures: false },
    Case { name: "literal-none", corpus: "english.txt", anre: "zebra", regex: "zebra", captures: false },
    Case { name: "alternation", corpus: "english.txt", anre: "apple|banana|cherry", regex: "apple|banana|cherry", captures: false },
    Case { name: "word-ing", corpus: "english.txt", anre: "[a-z]+ing", regex: "[a-z]+ing", captures: false },
    Case { name: "capital-word", corpus: "english.txt", anre: r"\b[A-Z][a-z]+\b", regex: r"(?-u:\b)[A-Z][a-z]+(?-u:\b)", captures: false },
    Case { name: "email", corpus: "english.txt", anre: r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}", regex: r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}", captures: false },
    Case { name: "date", corpus: "english.txt", anre: r"\d{4}-\d{2}-\d{2}", regex: r"(?-u:\d){4}-(?-u:\d){2}-(?-u:\d){2}", captures: false },
    Case { name: "date-captures", corpus: "english.txt", anre: r"(\d{4})-(\d{2})-(\d{2})", regex: r"((?-u:\d){4})-((?-u:\d){2})-((?-u:\d){2})", captures: true },
    Case { name: "ipv4", corpus: "english.txt", anre: r"(?:\d{1,3}\.){3}\d{1,3}", regex: r"(?:(?-u:\d){1,3}\.){3}(?-u:\d){1,3}", captures: false },
    Case { name: "hex-color", corpus: "english.txt", anre: "#[0-9a-fA-F]{6}", regex: "#[0-9a-fA-F]{6}", captures: false },
    Case { name: "html-tag", corpus: "html.txt", anre: "<([a-z]+)[^>]*>", regex: "<([a-z]+)[^>]*>", captures: true },
    Case { name: "html-attribute", corpus: "html.txt", anre: r#"class="[a-z]+""#, regex: r#"class="[a-z]+""#, captures: false },
    Case { name: "cjk-run", corpus: "unicode.txt", anre: "[一-龥]+", regex: "[一-龥]+", captures: false },
    Case { name: "cjk-literal", corpus: "unicode.txt", anre: "服务器", regex: "服务器", captures: false },
];

const MIN_RUNS: u32 = 3;
const TARGET_TIME: Duration = Duration::from_millis(500);

// Returns the fastest run and the matched byte total, which is compared across engines.
fn measure(mut search: impl FnMut() -> (usize, usize)) -> (Duration, usize, usize) {
    let (count, total) = search();
    let started = Instant::now();
    let mut best = Duration::MAX;
    let mut runs = 0;
    while runs < MIN_RUNS || started.elapsed() < TARGET_TIME {
        let run_started = Instant::now();
        black_box(search());
        best = best.min(run_started.elapsed());
        runs += 1;
    }
    (best, count, total)
}

fn compile_time(mut compile: impl FnMut()) -> Duration {
    const RUNS: u32 = 50;
    let started = Instant::now();
    for _ in 0..RUNS {
        compile();
    }
    started.elapsed() / RUNS
}

fn throughput(bytes: usize, time: Duration) -> f64 {
    bytes as f64 / (1024.0 * 1024.0) / time.as_secs_f64()
}

fn main() {
    let filter = std::env::args().nth(1).unwrap_or_default();
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");

    println!(
        "{:<15} {:>8} {:>11} {:>11} {:>11} {:>11} {:>9} {:>10} {:>10}",
        "case", "matches", "anre ms", "regex ms", "anre MB/s", "regex MB/s", "slower", "anre comp", "regex comp"
    );

    let mut ratios = vec![];
    for case in CASES.iter().filter(|case| case.name.contains(&filter)) {
        let text = fs::read_to_string(examples.join(case.corpus))
            .expect("missing corpus; run `cargo run --release --bin generate` first");

        let anre = regex_anre::Regex::new(case.anre).unwrap();
        let regex = regex::Regex::new(case.regex).unwrap();

        let (anre_time, anre_count, anre_total) = if case.captures {
            measure(|| {
                anre.captures_iter(&text).fold((0, 0), |(count, total), captures| {
                    let groups: usize = (1..captures.len()).filter_map(|i| captures.get(i)).map(|m| m.len()).sum();
                    (count + 1, total + groups + captures.get(0).unwrap().len())
                })
            })
        } else {
            measure(|| anre.find_iter(&text).fold((0, 0), |(count, total), m| (count + 1, total + m.len())))
        };

        let (regex_time, regex_count, regex_total) = if case.captures {
            measure(|| {
                regex.captures_iter(&text).fold((0, 0), |(count, total), captures| {
                    let groups: usize = (1..captures.len()).filter_map(|i| captures.get(i)).map(|m| m.len()).sum();
                    (count + 1, total + groups + captures.get(0).unwrap().len())
                })
            })
        } else {
            measure(|| regex.find_iter(&text).fold((0, 0), |(count, total), m| (count + 1, total + m.len())))
        };

        assert_eq!(
            (anre_count, anre_total),
            (regex_count, regex_total),
            "engines disagree on case {}",
            case.name
        );

        let anre_compile = compile_time(|| {
            black_box(regex_anre::Regex::new(case.anre).unwrap());
        });
        let regex_compile = compile_time(|| {
            black_box(regex::Regex::new(case.regex).unwrap());
        });

        let ratio = anre_time.as_secs_f64() / regex_time.as_secs_f64();
        ratios.push(ratio);
        println!(
            "{:<15} {:>8} {:>11.3} {:>11.3} {:>11.1} {:>11.1} {:>8.1}x {:>8.1}us {:>8.1}us",
            case.name,
            anre_count,
            anre_time.as_secs_f64() * 1000.0,
            regex_time.as_secs_f64() * 1000.0,
            throughput(text.len(), anre_time),
            throughput(text.len(), regex_time),
            ratio,
            anre_compile.as_secs_f64() * 1e6,
            regex_compile.as_secs_f64() * 1e6,
        );
    }

    if !ratios.is_empty() {
        let geometric_mean = (ratios.iter().map(|r| r.ln()).sum::<f64>() / ratios.len() as f64).exp();
        println!("\ngeometric mean: regex-anre is {geometric_mean:.1}x slower than regex");
    }
}
