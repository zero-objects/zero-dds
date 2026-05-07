// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Orchestriert Scanner + Lints + Reporter.
//!
//! Eintrittspunkt: [`run`].

use std::path::Path;

use anyhow::{Context, Result};

use crate::diagnostic::{Diagnostic, Severity};
use crate::lints::{FileLint, FileLintContext, default_crate_lints, default_file_lints};
use crate::scanner::{self, CrateInfo};

/// Konfiguration fuer einen Lint-Run.
#[derive(Debug, Clone)]
pub struct RunConfig {
    /// Workspace-Wurzel (enthaelt die Workspace-`Cargo.toml`).
    pub workspace_root: std::path::PathBuf,
    /// Wenn `true`, brechen Warnings ebenfalls den Run ab.
    pub fail_on_warning: bool,
}

/// Ergebnis eines Lint-Runs.
#[derive(Debug, Default)]
pub struct RunReport {
    /// Alle Findings (Errors + Warnings).
    pub diagnostics: Vec<Diagnostic>,
    /// Anzahl gescannter Crates.
    pub crates_scanned: usize,
    /// Anzahl gescannter Dateien.
    pub files_scanned: usize,
}

impl RunReport {
    /// Anzahl Errors.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }

    /// Anzahl Warnings.
    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count()
    }
}

/// Fuehrt alle Default-Lints auf dem Workspace aus.
///
/// # Errors
/// Fehler beim Lesen einzelner Quelldateien werden als Diagnose im Report
/// vermerkt; nur fundamentale I/O-Fehler (z.B. Workspace-Manifest fehlt)
/// brechen den Run als `Err` ab.
pub fn run(cfg: &RunConfig) -> Result<RunReport> {
    let crates = scanner::scan_workspace(&cfg.workspace_root)?;
    let file_lints = default_file_lints();
    let crate_lints = default_crate_lints();
    let mut report = RunReport {
        crates_scanned: crates.len(),
        ..RunReport::default()
    };

    for krate in &crates {
        for lint in &crate_lints {
            report.diagnostics.extend(lint.check(krate));
        }
        for file in &krate.source_files {
            report.files_scanned += 1;
            report
                .diagnostics
                .extend(check_file(file, krate, file_lints.as_slice()));
        }
    }
    Ok(report)
}

fn check_file(file: &Path, krate: &CrateInfo, lints: &[Box<dyn FileLint>]) -> Vec<Diagnostic> {
    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            return vec![Diagnostic::error(
                file,
                0,
                0,
                "dds_io_error",
                format!("konnte Datei nicht lesen: {e}"),
            )];
        }
    };
    let ast = match syn::parse_file(&source) {
        Ok(a) => a,
        Err(e) => {
            let span = e.span().start();
            return vec![Diagnostic::error(
                file,
                span.line,
                span.column.saturating_add(1),
                "dds_parse_error",
                format!("syn parse failed: {e}"),
            )];
        }
    };
    let ctx = FileLintContext {
        file,
        source: &source,
        ast: &ast,
        crate_class: krate.classification,
        crate_name: &krate.name,
    };
    let mut out = Vec::new();
    for lint in lints {
        out.extend(lint.check(&ctx));
    }
    out
}

/// Eingangsstelle fuer das CLI: ermittelt Workspace-Wurzel ueber
/// `cargo locate-project` falls noetig.
///
/// # Errors
/// Wenn `cargo` nicht erreichbar ist oder kein Workspace gefunden wird.
pub fn locate_workspace_root(start: &Path) -> Result<std::path::PathBuf> {
    // Direkter Weg: gibt es im Start-Pfad eine `Cargo.toml` mit `[workspace]`?
    let mut cur = start.to_path_buf();
    loop {
        let manifest = cur.join("Cargo.toml");
        if manifest.exists() {
            let content = std::fs::read_to_string(&manifest)
                .with_context(|| format!("read {}", manifest.display()))?;
            if content.contains("[workspace]") {
                return Ok(cur);
            }
        }
        if !cur.pop() {
            anyhow::bail!("kein Workspace-Cargo.toml gefunden ab {}", start.display());
        }
    }
}
