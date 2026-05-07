## Summary

<!-- Was ändert sich, und warum? Eine bis drei Sätze. -->

## Spec-Bezug

<!-- Welche OMG-Spec-Sektion ist betroffen? z.B. „XTypes 1.3 §7.4.2.4". -->
<!-- „N/A" wenn rein interner Refactor / Tooling. -->

## Test-Coverage

- [ ] Neue Tests für neuen Code geschrieben
- [ ] Bestehende Tests berücksichtigen die Änderung
- [ ] `cargo test --workspace` lokal grün
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` lokal grün
- [ ] `cargo fmt --all -- --check` lokal grün

## Breaking Change?

- [ ] Nein
- [ ] Ja — Migration-Notes hinzugefügt zum Crate-CHANGELOG.md

## Checklist

- [ ] Conventional-Commit-Title (`feat(scope): ...` / `fix(scope): ...` / `docs(scope): ...` / `chore(scope): ...`)
- [ ] Crate-CHANGELOG.md aktualisiert (falls Public-API betroffen)
- [ ] Spec-Coverage-Doc aktualisiert (falls Spec-Verhalten betroffen)
