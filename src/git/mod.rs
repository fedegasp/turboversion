//
// src/git/mod.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Confine verso git: tipi di dominio, il trait `GitRepo` e il backend
//! mock usato finché non c'è un repository reale (vedi roadmap in
//! README.md). Questo modulo non importa mai nulla da `app` o `ui`.

pub(crate) mod domain;
pub(crate) mod mock;
pub(crate) mod repo;
