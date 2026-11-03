//
// src/git/mod.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Confine verso git: tipi di dominio, il trait `Repo`, il backend reale
//! (`git2`, su `git2` esterno) e il backend mock (`mock`) usato dai test e
//! da `model_for_tests`/`TestApp`. Questo modulo non importa mai nulla da
//! `app` o `ui`.

pub(crate) mod domain;
pub(crate) mod git2;
// Usato solo da `model_for_tests`, `TestApp` e dai test di `effects.rs`/
// `worker.rs`: nessun codice di produzione lo referenzia più da quando
// `main.rs` sceglie `git2::GitRepo`.
#[cfg(test)]
pub(crate) mod mock;
pub(crate) mod repo;
