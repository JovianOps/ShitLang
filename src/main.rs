use std::collections::HashMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Command;

fn expand_path(p: &str) -> String {
    let home = std::env::var("USERPROFILE").or(std::env::var("HOME")).unwrap_or_default();
    p.replace("~", &home)
}

fn is_numeric(s: &str) -> bool {
    s.parse::<f64>().is_ok()
}

fn is_size_literal(s: &str) -> bool {
    let s = s.trim();
    let upper = s.to_uppercase();
    for suffix in ["TB", "GB", "MB", "KB", "B"] {
        if upper.ends_with(suffix) {
            let num_part = &s[..s.len() - suffix.len()];
            return is_numeric(num_part);
        }
    }
    false
}

fn interpolate(text: &str, scope: &Scope) -> String {
    let mut output = String::new();
    let mut rest = text;

    loop {
        if let Some(open) = rest.find('{') {
            if let Some(close) = rest[open+1..].find('}') {
                output.push_str(&rest[..open]);
                let var_name = &rest[open+1..open+1+close];
                output.push_str(&scope.get(var_name));
                rest = &rest[open+1+close+1..];
            } else {
                output.push_str(rest);
                break;
            }
        } else {
            output.push_str(rest);
            break;
        }
    }
    output
}

fn match_files(pattern: &str) -> Vec<String> {
    let expanded = expand_path(pattern);
    let mut files: Vec<String> = vec![];

    if let Some(star_pos) = expanded.find('*') {
        let before_star = expanded[..star_pos].trim_end_matches(|c: char| c == '/' || c == '\\');
        let dir_part = if before_star.is_empty() {
            ".".to_string()
        } else {
            let p = Path::new(before_star);
            if p.is_dir() {
                before_star.to_string()
            } else {
                p.parent().map(|par| par.to_string_lossy().to_string()).unwrap_or_else(|| ".".to_string())
            }
        };

        let ext_part = &expanded[star_pos..];
        let ext_filter: Option<String> = if ext_part.starts_with("*.") {
            Some(ext_part[2..].to_string())
        } else {
            None
        };

        let path = Path::new(&dir_part);
        if path.is_dir() {
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() {
                        if let Some(ext) = &ext_filter {
                            if p.extension().map_or(false, |e| e == std::ffi::OsStr::new(ext)) {
                                files.push(p.to_str().unwrap().to_string());
                            }
                        } else {
                            files.push(p.to_str().unwrap().to_string());
                        }
                    }
                }
            }
        }
    } else {
        let p = Path::new(&expanded);
        if p.is_file() {
            files.push(expanded);
        }
    }

    files.sort();
    files
}

fn parse_size(s: &str) -> i64 {
    let s = s.trim();
    let upper = s.to_uppercase();

    let (num_str, mult) = if upper.ends_with("KB") { (&s[..s.len()-2], 1024i64) }
        else if upper.ends_with("MB") { (&s[..s.len()-2], 1024i64*1024) }
        else if upper.ends_with("GB") { (&s[..s.len()-2], 1024i64*1024*1024) }
        else if upper.ends_with("TB") { (&s[..s.len()-2], 1024i64*1024*1024*1024) }
        else if upper.ends_with("K") { (&s[..s.len()-1], 1024i64) }
        else if upper.ends_with("M") { (&s[..s.len()-1], 1024i64*1024) }
        else if upper.ends_with("G") { (&s[..s.len()-1], 1024i64*1024*1024) }
        else if upper.ends_with("T") { (&s[..s.len()-1], 1024i64*1024*1024*1024) }
        else if upper.ends_with("B") { (&s[..s.len()-1], 1i64) }
        else { (s, 1i64) };

    num_str.parse::<f64>().unwrap_or(0.0) as i64 * mult
}

fn dir_size(dir: &str) -> i64 {
    let mut total: i64 = 0;
    let path = Path::new(dir);
    if path.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    if let Ok(meta) = p.metadata() {
                        total += meta.len() as i64;
                    }
                }
            }
        }
    }
    total
}

#[derive(Clone)]
struct Scope {
    vars: HashMap<String, String>,
    parent: Option<Box<Scope>>,
}

impl Scope {
    fn new() -> Self {
        Scope { vars: HashMap::new(), parent: None }
    }

    fn with_parent(parent: &Scope) -> Self {
        Scope { vars: HashMap::new(), parent: Some(Box::new(parent.clone())) }
    }

    fn get(&self, name: &str) -> String {
        if let Some(v) = self.vars.get(name) {
            return v.clone();
        }
        if let Some(p) = &self.parent {
            return p.get(name);
        }
        name.to_string()
    }

    fn set(&mut self, name: &str, value: String) {
        self.vars.insert(name.to_string(), value);
    }
}

fn eval_expr(expr: &str, scope: &Scope) -> String {
    if expr.to_lowercase().starts_with("count ") {
        let pattern = expr[6..].trim().trim_matches('"');
        let pattern = interpolate(pattern, scope);
        return match_files(&pattern).len().to_string();
    }

    if expr.to_lowercase().starts_with("size ") {
        let pattern = expr[5..].trim();
        let p = expand_path(&interpolate(pattern, scope));
        let path = Path::new(&p);
        let sz = if path.is_dir() { dir_size(&p) }
            else if path.is_file() { path.metadata().map(|m| m.len() as i64).unwrap_or(0) }
            else { 0 };
        return sz.to_string();
    }

    if is_size_literal(expr) {
        return parse_size(expr).to_string();
    }

    if is_numeric(expr) {
        return expr.to_string();
    }

    if expr.len() >= 2 && expr.starts_with('"') && expr.ends_with('"') {
        let inner = &expr[1..expr.len()-1];
        return interpolate(inner, scope);
    }

    if expr.contains('{') && expr.contains('}') {
        return interpolate(expr, scope);
    }

    expr.to_string()
}

fn eval_condition(cond: &str, scope: &Scope) -> bool {
    let ops = ["==", "!=", ">=", "<=", ">", "<"];

    for op in &ops {
        if cond.contains(op) {
            let parts: Vec<&str> = cond.splitn(2, op).collect();
            let left_val = eval_expr(parts[0].trim(), scope);
            let right_val = eval_expr(parts[1].trim(), scope);

            if let (Ok(l), Ok(r)) = (left_val.parse::<f64>(), right_val.parse::<f64>()) {
                return match *op {
                    "==" => l == r,
                    "!=" => l != r,
                    ">=" => l >= r,
                    "<=" => l <= r,
                    ">"  => l > r,
                    "<"  => l < r,
                    _ => false,
                };
            }
            return match *op {
                "==" => left_val == right_val,
                "!=" => left_val != right_val,
                _ => false,
            };
        }
    }

    if cond.to_lowercase().contains("exists") {
        let cleaned = cond.to_lowercase().replace("exists", "").trim().to_string();
        let cleaned = interpolate(&cleaned, scope);
        return Path::new(&expand_path(&cleaned)).exists();
    }

    let val = eval_expr(cond, scope);
    !val.is_empty()
}

fn unescape(s: &str) -> String {
    let mut output = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                match next {
                    'n' => { chars.next(); output.push('\n'); }
                    't' => { chars.next(); output.push('\t'); }
                    '"' => { chars.next(); output.push('"'); }
                    '\\' => { chars.next(); output.push('\\'); }
                    _ => { output.push('\\'); }
                }
            } else {
                output.push('\\');
            }
        } else {
            output.push(c);
        }
    }
    output
}

fn do_file_op(op: &str, args: &str, scope: &Scope) {
    match op {
        "delete" => {
            let pattern = args.trim_start_matches("delete ").trim().trim_matches('"');
            let pattern = interpolate(pattern, scope);
            for f in match_files(&pattern) {
                let _ = fs::remove_file(&f);
            }
        }
        "rename" => {
            if let Some(idx) = args.to_lowercase().find(" to ") {
                let pattern = args[..idx].trim_start_matches("rename ").trim().trim_matches('"');
                let template = args[idx+4..].trim().trim_matches('"');
                let pattern = interpolate(&pattern, scope);

                let files = match_files(&pattern);
                if files.is_empty() { return; }

                let first_path = Path::new(&files[0]);
                let dir = first_path.parent().map(|p| p.to_str().unwrap_or(".")).unwrap_or(".");
                let ext_template = first_path.extension().map(|e| e.to_str().unwrap_or("")).unwrap_or("");

                for (i, f) in files.iter().enumerate() {
                    let n = format!("{:03}", i + 1);
                    let new_name = template.replace("{n}", &n);

                    let new_name = if template.contains("{n}") && !ext_template.is_empty() && !new_name.ends_with(ext_template) {
                        format!("{}.{}", new_name, ext_template)
                    } else {
                        new_name
                    };

                    let new_path = Path::new(dir).join(&new_name);
                    let _ = fs::rename(f, &new_path);
                }
            }
        }
        "move" => {
            if let Some(idx) = args.to_lowercase().find(" to ") {
                let pattern = args[..idx].trim_start_matches("move").trim().trim_matches('"');
                let dest = expand_path(&interpolate(args[idx+4..].trim().trim_matches('"'), scope));
                let _ = fs::create_dir_all(&dest);
                for f in match_files(&interpolate(&pattern, scope)) {
                    let fname = Path::new(&f).file_name().unwrap().to_str().unwrap();
                    let _ = fs::rename(&f, Path::new(&dest).join(fname));
                }
            }
        }
        "copy" => {
            if let Some(idx) = args.to_lowercase().find(" to ") {
                let pattern = args[..idx].trim_start_matches("copy").trim().trim_matches('"');
                let dest = expand_path(&interpolate(args[idx+4..].trim().trim_matches('"'), scope));
                let _ = fs::create_dir_all(&dest);
                for f in match_files(&interpolate(&pattern, scope)) {
                    let fname = Path::new(&f).file_name().unwrap().to_str().unwrap();
                    let _ = fs::copy(&f, Path::new(&dest).join(fname));
                }
            }
        }
        _ => {}
    }
}

fn run_system_op(op: &str, args: &str, scope: &mut Scope) {
    match op {
        "say" | "print" => {
            let msg = args.trim_start_matches("say ").trim_start_matches("print ").trim().trim_matches('"');
            let msg = interpolate(msg, scope);
            println!("{}", msg);
        }
        "run" => {
            let cmd = args.trim_start_matches("run ").trim().trim_matches('"');
            let cmd = interpolate(cmd, scope);
            let _ = Command::new("cmd").args(&["/C", &cmd]).status();
        }
        "notify" => {
            let msg = args.trim().trim_matches('"');
            let msg = interpolate(msg, scope);
            let _ = Command::new("cmd").args(&["/C", &format!("msg * \"{}\"", msg)]).status();
        }
        "http" => {
            let url = args.trim().trim_matches('"');
            let url = interpolate(url, scope);
            println!("HTTP GET: {}", url);
        }
        _ => {}
    }
}

fn execute_block(block_str: &str, scope: &mut Scope) {
    for line in block_str.split('\n') {
        if line.trim().is_empty() || line.trim().starts_with('#') {
            continue;
        }
        execute_line(line, scope);
    }
}

fn execute_line(line: &str, scope: &mut Scope) {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return;
    }

    let lower = line.to_lowercase();

    let file_ops = ["rename ", "move ", "delete ", "copy "];
    for op in file_ops {
        if lower.starts_with(op) {
            do_file_op(&op.trim(), &line[op.len()..], scope);
            return;
        }
    }

    if lower.starts_with("write ") {
        if let Some(idx) = line.find(" to ") {
            let content = line[6..idx].trim().trim_matches('"');
            let filepath = interpolate(line[idx+4..].trim().trim_matches('"'), scope);
            let content = unescape(&interpolate(content, scope));
            let _ = fs::write(&filepath, content);
            return;
        }
    }
    if lower.starts_with("append ") {
        if let Some(idx) = line.find(" to ") {
            let content = line[7..idx].trim().trim_matches('"');
            let filepath = interpolate(line[idx+4..].trim().trim_matches('"'), scope);
            let content = unescape(&interpolate(content, scope));
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&filepath) {
                let _ = f.write_all(content.as_bytes());
            }
            return;
        }
    }

    let sys_ops = ["run ", "notify ", "http ", "say ", "print "];
    for op in sys_ops {
        if lower.starts_with(op) {
            run_system_op(&op.trim(), &line[op.len()..], scope);
            return;
        }
    }

    if lower.starts_with("count ") {
        let result = eval_expr(line, scope);
        println!("{}", result);
        return;
    }

    if lower.starts_with("set ") {
        if let Some(idx) = line[4..].find('=') {
            let var = line[4..4+idx].trim();
            let val = eval_expr(line[4+idx+1..].trim(), scope);
            scope.set(var, val);
            return;
        }
    }

    if let Some(idx) = line.find('=') {
        let var = line[..idx].trim();
        let val = eval_expr(line[idx+1..].trim(), scope);
        scope.set(var, val);
        return;
    }

    if lower.starts_with("if ") || lower.starts_with("when ") || lower.starts_with("unless ") {
        let prefix_len = if lower.starts_with("if ") { 3 }
            else if lower.starts_with("when ") { 5 }
            else { 7 };
        if let Some(colon_idx) = line.find(" : ") {
            let cond = &line[prefix_len..colon_idx];
            let body = &line[colon_idx+3..];
            let result = if lower.starts_with("unless ") {
                !eval_condition(cond, scope)
            } else {
                eval_condition(cond, scope)
            };
            if result {
                execute_line(body, scope);
            }
            return;
        }
    }

    if lower.starts_with("for each ") {
        let rest = &line[9..];
        if let Some(in_idx) = rest.to_lowercase().find(" in ") {
            if let Some(colon_idx) = rest.find(" : ") {
                let var = rest[..in_idx].trim();
                let pattern = interpolate(rest[in_idx+4..colon_idx].trim(), scope);
                let body = &rest[colon_idx+3..];
                for f in match_files(&pattern) {
                    let mut loop_scope = Scope::with_parent(scope);
                    loop_scope.set(var, f.clone());
                    execute_line(body, &mut loop_scope);
                }
            }
        }
        return;
    }

    if lower.starts_with("while ") {
        if let Some(colon_idx) = line.find(" : ") {
            let cond_str = &line[6..colon_idx];
            let body = &line[colon_idx+3..];
            let mut guard = 0;
            while eval_condition(cond_str, scope) && guard < 1000 {
                execute_line(body, scope);
                guard += 1;
            }
        }
        return;
    }

    if lower.starts_with("every ") || lower.starts_with("at ") {
        println!("Scheduled: {}", interpolate(line, scope));
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: shitlang <script.shit>");
        eprintln!("       shitlang --eval \"rename *.txt to backup_{{n}}\"");
        std::process::exit(1);
    }

    let mut scope = Scope::new();

    if args[1] == "--eval" {
        if args.len() < 3 {
            eprintln!("Error: --eval requires a code argument");
            std::process::exit(1);
        }
        execute_block(&args[2], &mut scope);
    } else {
        let filepath = &args[1];
        match fs::read_to_string(filepath) {
            Ok(content) => {
                execute_block(&content, &mut scope);
            }
            Err(e) => {
                eprintln!("Error reading {}: {}", filepath, e);
                std::process::exit(1);
            }
        }
    }
}
