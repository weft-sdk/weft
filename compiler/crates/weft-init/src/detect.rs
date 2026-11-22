//! Recognising what kind of project a folder is.

use std::fmt;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    TypeScript,
}

impl Language {
    /// The name used in `weft.yaml`.
    pub fn id(self) -> &'static str {
        match self {
            Language::TypeScript => "typescript",
        }
    }

    /// Parse a language name as typed on the command line. `None` for names Weft does not know.
    pub fn parse(name: &str) -> Option<Language> {
        match name.to_lowercase().as_str() {
            "typescript" | "ts" => Some(Language::TypeScript),
            _ => None,
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Language::TypeScript => f.write_str("TypeScript"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
    Deno,
}

impl fmt::Display for PackageManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PackageManager::Npm => "npm",
            PackageManager::Pnpm => "pnpm",
            PackageManager::Yarn => "yarn",
            PackageManager::Bun => "bun",
            PackageManager::Deno => "deno",
        })
    }
}

impl PackageManager {
    /// The program and arguments that add `packages` to a project.
    pub fn add_command(self, packages: &[String]) -> (String, Vec<String>) {
        let mut args: Vec<String> = match self {
            PackageManager::Npm => vec!["install".into()],
            PackageManager::Deno => vec!["add".into()],
            _ => vec!["add".into()],
        };
        for p in packages {
            // Deno takes npm packages with a prefix.
            args.push(if self == PackageManager::Deno {
                format!("npm:{p}")
            } else {
                p.clone()
            });
        }
        (self.to_string(), args)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Detected {
    pub language: Option<Language>,
    pub package_manager: Option<PackageManager>,
}

/// Look at the files in `dir`. An empty folder detects nothing.
pub fn detect(dir: &Path) -> Detected {
    let has = |name: &str| dir.join(name).exists();
    let language =
        (has("package.json") || has("tsconfig.json") || has("deno.json") || has("deno.jsonc"))
            .then_some(Language::TypeScript);
    // Lock files are the clearest sign of which tool the project uses.
    let package_manager = if has("pnpm-lock.yaml") {
        Some(PackageManager::Pnpm)
    } else if has("yarn.lock") {
        Some(PackageManager::Yarn)
    } else if has("bun.lock") || has("bun.lockb") {
        Some(PackageManager::Bun)
    } else if has("deno.json") || has("deno.jsonc") || has("deno.lock") {
        Some(PackageManager::Deno)
    } else if has("package-lock.json") || has("package.json") {
        // npm is what a package.json means when nothing else says otherwise.
        Some(PackageManager::Npm)
    } else {
        None
    };
    Detected {
        language,
        package_manager,
    }
}
