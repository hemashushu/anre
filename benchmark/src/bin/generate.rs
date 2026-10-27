// Generates the deterministic benchmark corpora in `../examples`.
//
// Run with: `cargo run --release --bin generate`

use std::{fs, path::Path};

const WORDS: &[&str] = &[
    "the", "of", "and", "to", "in", "is", "that", "it", "was", "for", "on", "are", "as", "with",
    "they", "be", "at", "one", "have", "this", "from", "by", "hot", "word", "but", "what", "some",
    "we", "can", "out", "other", "were", "all", "there", "when", "up", "use", "your", "how", "said",
    "an", "each", "she", "which", "do", "their", "time", "if", "will", "way", "about", "many",
    "then", "them", "write", "would", "like", "so", "these", "her", "long", "make", "thing", "see",
    "him", "two", "has", "look", "more", "day", "could", "go", "come", "did", "number", "sound",
    "no", "most", "people", "my", "over", "know", "water", "than", "call", "first", "who", "may",
    "down", "side", "been", "now", "find", "engine", "pattern", "matching", "parser", "compiler",
    "string", "character", "repeat", "group", "capture", "memory", "network", "server", "client",
    "request", "response", "running", "testing", "building", "reading", "writing", "searching",
    "apple", "banana", "cherry", "river", "mountain", "garden", "window", "letter", "station",
];

const NAMES: &[&str] = &[
    "alice", "bob", "carol", "dave", "erin", "frank", "grace", "heidi", "ivan", "judy", "mallory",
    "oscar", "peggy", "trent", "victor", "walter",
];

const DOMAINS: &[&str] = &[
    "example.com", "mail.example.org", "test.net", "company.io", "university.edu",
];

const TAGS: &[&str] = &["div", "span", "p", "a", "li", "ul", "section", "article", "em", "strong"];

const CHINESE: &[&str] = &[
    "正则表达式", "引擎", "匹配", "文本", "字符", "回溯", "编译器", "解析器", "性能", "测试",
    "数据", "网络", "服务器", "客户端", "我们", "今天", "天气", "很好", "学习", "程序",
];

const EMOJIS: &[&str] = &["😀", "🚀", "🌍", "❤", "🎉", "📚"];

// A small linear congruential generator keeps the output identical on every run.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

fn special_token(random: &mut Random) -> String {
    match random.below(6) {
        0 => format!(
            "{}.{}@{}",
            random.pick(NAMES),
            random.pick(NAMES),
            random.pick(DOMAINS)
        ),
        1 => format!(
            "{}-{:02}-{:02}",
            1990 + random.below(40),
            1 + random.below(12),
            1 + random.below(28)
        ),
        2 => format!(
            "{}.{}.{}.{}",
            random.below(256),
            random.below(256),
            random.below(256),
            random.below(256)
        ),
        3 => format!("#{:06x}", random.next() & 0xff_ffff),
        4 => format!("{}", random.next() % 100_000),
        _ => format!("https://{}/{}", random.pick(DOMAINS), random.pick(WORDS)),
    }
}

fn english(random: &mut Random, size: usize) -> String {
    let mut text = String::with_capacity(size + 256);
    while text.len() < size {
        let sentence_length = 8 + random.below(13);
        for index in 0..sentence_length {
            let word = if random.below(25) == 0 {
                special_token(random)
            } else {
                random.pick(WORDS).to_owned()
            };
            if index == 0 {
                let mut chars = word.chars();
                if let Some(first) = chars.next() {
                    text.extend(first.to_uppercase());
                    text.push_str(chars.as_str());
                }
            } else {
                text.push(' ');
                text.push_str(&word);
            }
        }
        text.push_str(if random.below(5) == 0 { "!" } else { "." });
        text.push_str(if random.below(6) == 0 { "\n\n" } else { " " });
    }
    text
}

fn html(random: &mut Random, size: usize) -> String {
    let mut text = String::with_capacity(size + 256);
    text.push_str("<html>\n<body>\n");
    while text.len() < size {
        let tag = random.pick(TAGS);
        text.push('<');
        text.push_str(tag);
        if random.below(2) == 0 {
            text.push_str(&format!(" class=\"{}\"", random.pick(WORDS)));
        }
        if random.below(4) == 0 {
            text.push_str(&format!(" id=\"{}{}\"", random.pick(WORDS), random.below(1000)));
        }
        text.push('>');
        for _ in 0..(1 + random.below(8)) {
            text.push_str(random.pick(WORDS));
            text.push(' ');
        }
        text.push_str("</");
        text.push_str(tag);
        text.push_str(">\n");
    }
    text.push_str("</body>\n</html>\n");
    text
}

fn unicode(random: &mut Random, size: usize) -> String {
    let mut text = String::with_capacity(size + 256);
    while text.len() < size {
        match random.below(10) {
            0..=5 => text.push_str(random.pick(CHINESE)),
            6..=8 => {
                text.push(' ');
                text.push_str(random.pick(WORDS));
                text.push(' ');
            }
            _ => text.push_str(random.pick(EMOJIS)),
        }
        if random.below(12) == 0 {
            text.push_str(if random.below(3) == 0 { "。\n" } else { "，" });
        }
    }
    text
}

fn main() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
    fs::create_dir_all(&directory).unwrap();

    let mut random = Random(2026);
    let corpora = [
        ("english.txt", english(&mut random, 2 * 1024 * 1024)),
        ("html.txt", html(&mut random, 1024 * 1024)),
        ("unicode.txt", unicode(&mut random, 1024 * 1024)),
    ];

    for (name, text) in corpora {
        let path = directory.join(name);
        fs::write(&path, text).unwrap();
        println!("wrote {}", path.display());
    }
}
