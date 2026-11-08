#![allow(dead_code)]

mod ast;
mod codegen;
mod lexer;
mod parser;
mod preprocess;
mod sema;
mod types;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn usage() {
    eprintln!(
        "Fallen derleyici v0.3\n\
         Kullanım:\n\
         \x20 fallen build  <dosya.flen> [-o çıktı] [-S] [--stats] [--keep-asm]\n\
         \x20 fallen run    <dosya.flen> [--stats]\n\
         \x20 fallen check  <dosya.flen>              (kod üretmeden semantik kontrol)\n\
         \x20 fallen version\n\n\
         C sözdizimi + Rust seviyesi bellek koruması + OOP + switch/case + doğrudan makine kodu."
    );
}

struct Options {
    command: String,
    input: PathBuf,
    output: Option<PathBuf>,
    emit_asm_only: bool,
    stats: bool,
    keep_asm: bool,
}

fn parse_args() -> Result<Options, String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or_else(|| "komut eksik".to_string())?;
    if command == "-h" || command == "--help" || command == "help" {
        usage();
        std::process::exit(0);
    }
    if command == "--version" || command == "version" {
        println!("fallen 0.3.0");
        std::process::exit(0);
    }
    if command != "build" && command != "run" && command != "compile" && command != "check" {
        if command.ends_with(".flen") {
            let mut rest = vec![command.clone()];
            rest.extend(args);
            let mut it = rest.into_iter();
            let input = PathBuf::from(it.next().unwrap());
            let mut opts = Options {
                command: "build".into(),
                input,
                output: None,
                emit_asm_only: false,
                stats: false,
                keep_asm: false,
            };
            for a in it {
                match a.as_str() {
                    "-o" => {}
                    "-S" => opts.emit_asm_only = true,
                    "--stats" => opts.stats = true,
                    "--keep-asm" => opts.keep_asm = true,
                    _ => {}
                }
            }
            return Ok(opts);
        }
        return Err(format!("bilinmeyen komut: {}", command));
    }

    let input = args
        .next()
        .ok_or_else(|| "girdi dosyası eksik".to_string())?;
    let mut opts = Options {
        command,
        input: PathBuf::from(input),
        output: None,
        emit_asm_only: false,
        stats: false,
        keep_asm: false,
    };
    while let Some(a) = args.next() {
        match a.as_str() {
            "-o" => {
                let o = args.next().ok_or_else(|| "-o için çıktı adı gerekli".to_string())?;
                opts.output = Some(PathBuf::from(o));
            }
            "-S" => opts.emit_asm_only = true,
            "--stats" => opts.stats = true,
            "--keep-asm" => opts.keep_asm = true,
            other => return Err(format!("bilinmeyen seçenek: {}", other)),
        }
    }
    Ok(opts)
}

fn default_output(input: &Path) -> PathBuf {
    let mut p = input.to_path_buf();
    p.set_extension("");
    p
}

fn first_existing(paths: &[&str]) -> Option<String> {
    paths.iter().find(|p| Path::new(p).exists()).map(|p| p.to_string())
}

fn link_direct(obj: &Path, out: &Path) -> Result<bool, String> {
    let crt1 = match first_existing(&[
        "/usr/lib64/crt1.o",
        "/usr/lib/x86_64-linux-gnu/crt1.o",
        "/usr/lib/crt1.o",
    ]) {
        Some(p) => p,
        None => return Ok(false),
    };
    let crti = match first_existing(&[
        "/usr/lib64/crti.o",
        "/usr/lib/x86_64-linux-gnu/crti.o",
        "/usr/lib/crti.o",
    ]) {
        Some(p) => p,
        None => return Ok(false),
    };
    let crtn = match first_existing(&[
        "/usr/lib64/crtn.o",
        "/usr/lib/x86_64-linux-gnu/crtn.o",
        "/usr/lib/crtn.o",
    ]) {
        Some(p) => p,
        None => return Ok(false),
    };
    let loader = match first_existing(&[
        "/lib64/ld-linux-x86-64.so.2",
        "/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2",
        "/lib/ld-linux-x86-64.so.2",
    ]) {
        Some(p) => p,
        None => return Ok(false),
    };

    let status = Command::new("ld")
        .arg("-o")
        .arg(out)
        .arg(&crt1)
        .arg(&crti)
        .arg(obj)
        .arg("-lc")
        .arg("-dynamic-linker")
        .arg(&loader)
        .arg(&crtn)
        .arg("-L/usr/lib64")
        .arg("-L/lib64")
        .arg("-L/usr/lib/x86_64-linux-gnu")
        .arg("-L/lib/x86_64-linux-gnu")
        .status()
        .map_err(|e| format!("'ld' çalıştırılamadı: {}", e))?;
    if !status.success() {
        return Err("doğrudan bağlama (ld) başarısız oldu".into());
    }
    Ok(true)
}

fn main() {
    if let Err(e) = real_main() {
        eprintln!("Fallen: {}", e);
        std::process::exit(1);
    }
}

fn real_main() -> Result<(), String> {
    let opts = parse_args()?;
    let src = fs::read_to_string(&opts.input)
        .map_err(|e| format!("{} okunamadı: {}", opts.input.display(), e))?;

    let t0 = Instant::now();
    let toks = preprocess::preprocess(&src).map_err(|e| format!("önişlemci hatası: {}", e))?;
    let t_lex = t0.elapsed();

    let mut program = parser::Parser::new(toks)
        .parse_program()
        .map_err(|e| format!("ayrıştırma hatası: {}", e))?;
    let t_parse = t0.elapsed();

    let mut sema = sema::Sema::new();
    sema.analyze(&mut program)
        .map_err(|e| format!("anlamsal hata: {}", e))?;
    let t_sema = t0.elapsed();

    // "check" komutu: sadece semantik analiz, kod üretme yok
    if opts.command == "check" {
        eprintln!(
            "Fallen: {} — semantik kontrol OK ({} ms)",
            opts.input.display(),
            t_sema.as_millis()
        );
        if opts.stats {
            eprintln!("\n--- Fallen anlamsal analiz ---");
            eprintln!("lexer+ön  : {:?}", t_lex);
            eprintln!("parser   : {:?}", t_parse.saturating_sub(t_lex));
            eprintln!("semantik : {:?}", t_sema.saturating_sub(t_parse));
            for c in sema.classes.values() {
                eprintln!(
                    "sınıf {:<16} boyut={:>4} bayt  hizalama={}  alan={}  metot={}",
                    c.name,
                    c.size,
                    c.align,
                    c.field_order.len(),
                    c.methods.len()
                );
            }
        }
        return Ok(());
    }

    let cg = codegen::Codegen::new(&sema.classes, &sema.funcs);
    let asm = cg
        .generate(&program)
        .map_err(|e| format!("kod üretim hatası: {}", e))?;
    let t_codegen = t0.elapsed();

    let is_run = opts.command == "run";
    let out = if is_run && opts.output.is_none() {
        let stem = opts
            .input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("out");
        env::temp_dir().join(format!("fallen_run_{}_{}", std::process::id(), stem))
    } else {
        opts
            .output
            .clone()
            .unwrap_or_else(|| default_output(&opts.input))
    };

    if opts.emit_asm_only {
        let asm_path = out.with_extension("s");
        fs::write(&asm_path, &asm)
            .map_err(|e| format!("{} yazılamadı: {}", asm_path.display(), e))?;
        eprintln!("Fallen: {}", asm_path.display());
    } else {
        let stem = opts
            .input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("out");
        let tmp = env::temp_dir().join(format!(
            "fallen_{}_{}",
            std::process::id(),
            stem
        ));
        let asm_path = tmp.with_extension("s");
        let obj_path = tmp.with_extension("o");
        fs::write(&asm_path, &asm)
            .map_err(|e| format!("geçici asm yazılamadı: {}", e))?;

        let status = Command::new("as")
            .arg("--64")
            .arg("-o")
            .arg(&obj_path)
            .arg(&asm_path)
            .status()
            .map_err(|e| format!("'as' çalıştırılamadı: {}", e))?;
        if !status.success() {
            return Err("assembler (as) başarısız oldu".into());
        }

        if !link_direct(&obj_path, &out)? {
            let status = Command::new("cc")
                .args(["-no-pie", "-o"])
                .arg(&out)
                .arg(&obj_path)
                .status()
                .map_err(|e| format!("bağlayıcı çalıştırılamadı: {}", e))?;
            if !status.success() {
                return Err("bağlama (link) başarısız oldu".into());
            }
        }

        if !opts.keep_asm {
            let _ = fs::remove_file(&asm_path);
        } else {
            let keep = out.with_extension("s");
            let _ = fs::copy(&asm_path, &keep);
            eprintln!("Fallen: assembly korundu -> {}", keep.display());
        }
        let _ = fs::remove_file(&obj_path);
        eprintln!("Fallen: {} -> {}", opts.input.display(), out.display());
    }

    if opts.stats {
        eprintln!("\n--- Fallen derleme & bellek analizi ---");
        eprintln!("lexer    : {:?}", t_lex);
        eprintln!("parser   : {:?}", t_parse.saturating_sub(t_lex));
        eprintln!("semantik : {:?}", t_sema.saturating_sub(t_parse));
        eprintln!("kodgen   : {:?}", t_codegen.saturating_sub(t_sema));
        eprintln!("toplam   : {:?}", t_codegen);
        for c in sema.classes.values() {
            eprintln!(
                "sınıf {:<16} boyut={:>4} bayt  hizalama={}  alan={}  metot={}",
                c.name,
                c.size,
                c.align,
                c.field_order.len(),
                c.methods.len()
            );
        }
        let mut total = 0usize;
        for r in &sema.report {
            total += r.frame_size;
            eprintln!(
                "fonk  {:<17} çerçeve={:>5} bayt  yerel={:>5}  toplam={:>5}",
                r.function, r.frame_size, r.locals, r.aggregate_bytes
            );
        }
        eprintln!("statik toplam stack rezervi: {} bayt", total);
    }

    if is_run {
        let status = Command::new(&out)
            .status()
            .map_err(|e| format!("{} çalıştırılamadı: {}", out.display(), e))?;
        if opts.output.is_none() {
            let _ = fs::remove_file(&out);
        }
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
