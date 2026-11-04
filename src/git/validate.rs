//
// src/git/validate.rs
// TurboVersion
//
// 2026-11-03 Federico Gasperini
//

//! Regole di validazione e messaggi d'errore condivisi dai backend `Repo`.
//! Stanno qui, e non dentro un backend, perché la regola deve essere una
//! sola: un nome rifiutato dal repository reale deve essere rifiutato anche
//! dal mock, altrimenti i test della UI verificherebbero un comportamento
//! che in produzione non esiste.

use super::domain::{Branch, BranchKind};
use super::repo::GitError;

/// Valida un nome di ramo applicando le regole di `git check-ref-format`
/// che contano per un nome digitato a mano. Restituisce il messaggio da
/// mostrare quando il nome non va bene, `None` quando è accettabile.
///
/// La validazione anticipa quella di libgit2 invece di delegarle tutto, per
/// tre motivi: serve il controllo sui duplicati, i messaggi di libgit2 sono
/// testo interno della libreria e non vanno mostrati in un dialog, e
/// soprattutto libgit2 è più permissivo di `git branch` — accetta nomi come
/// `a//b` o `/lead` *normalizzandoli* in silenzio in `a/b` e `lead`,
/// creando quindi un ramo con un nome diverso da quello richiesto.
pub(crate) fn validate_branch_name(name: &str, existing: &[Branch]) -> Option<String> {
    const INVALID_CHARS: &[char] = &['~', '^', ':', '?', '*', '[', '\\', ' '];
    if name.is_empty() {
        return Some("Branch name cannot be empty.".to_string());
    }
    if name.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Some("Branch names cannot contain spaces or control characters.".to_string());
    }
    // Un `-` iniziale eviterebbe che un domani, passato a `git` come
    // argomento, il nome venga scambiato per un'opzione.
    if name.starts_with('-') {
        return Some("Branch names cannot start with '-'.".to_string());
    }
    // `HEAD`, il suo alias `@` e un nome già qualificato confonderebbero il
    // riferimento con sé stesso: `refs/heads/refs/heads/x` non è quello che
    // l'utente chiede, e `@` da solo significa già HEAD.
    if name == "HEAD" || name == "@" || name.starts_with("refs/") {
        return Some("'HEAD', '@' and names starting with 'refs/' are reserved.".to_string());
    }
    if name.contains("..")
        || name.contains("@{")
        || name.chars().any(|c| INVALID_CHARS.contains(&c))
    {
        return Some(
            "Branch names cannot contain '..', '@{', or any of ~ ^ : ? * [ \\.".to_string(),
        );
    }
    // Le regole seguenti valgono per ogni componente fra `/`, non solo per
    // il nome intero: `a//b`, `/lead` e `a/.b` sono tutti casi che git
    // rifiuta e che libgit2 invece accetta riscrivendoli.
    for part in name.split('/') {
        if part.is_empty() {
            return Some(
                "Branch names cannot start or end with '/', nor contain '//'.".to_string(),
            );
        }
        if part.starts_with('.') || part.ends_with('.') || part.ends_with(".lock") {
            return Some(
                "No part of a branch name can start with '.' or end with '.' or '.lock'."
                    .to_string(),
            );
        }
    }
    if let Some(clash) = existing.iter().find(|b| b.name == name) {
        // Il nome esatto collide anche con tag e rami remoti: git stesso
        // avverte di riferimento ambiguo in quei casi. Dire "branch already
        // exists" manderebbe però l'utente a cercare un ramo che non c'è.
        return Some(match clash.kind {
            BranchKind::Local => branch_already_exists(name).0,
            BranchKind::Remote => {
                format!("A remote-tracking branch named '{name}' already exists.")
            }
            BranchKind::Tag => format!("A tag named '{name}' already exists."),
        });
    }
    // Un ramo è un riferimento con un percorso: `a` e `a/b` non possono
    // coesistere, perché `refs/heads/a` dovrebbe essere allo stesso tempo
    // un file e una directory. Il controllo sta qui e non nella mappatura
    // degli errori del backend perché copre entrambi i versi con un
    // messaggio solo, non dipende da come sono memorizzati i riferimenti
    // (sciolti o impacchettati) e vale anche per il mock. Solo rami locali:
    // tag e rami remoti vivono in altri percorsi e non collidono.
    if let Some(other) = existing.iter().find(|b| {
        b.kind == BranchKind::Local
            && (b.name.starts_with(&format!("{name}/"))
                || name.starts_with(&format!("{}/", b.name)))
    }) {
        return Some(branch_path_conflict(name, &other.name).0);
    }
    None
}

/// Errore per un nome che non può coesistere con un ramo esistente perché
/// uno è il prefisso di percorso dell'altro (`a` e `a/b`).
pub(crate) fn branch_path_conflict(name: &str, other: &str) -> GitError {
    GitError(format!(
        "Branch '{name}' conflicts with the existing branch '{other}'."
    ))
}

/// Errore per la creazione di un ramo il cui nome è già preso. Lo usa anche
/// il backend reale quando il conflitto emerge solo alla scrittura (es. su
/// un filesystem case-insensitive, dove `feature` e `FEATURE` sono lo
/// stesso riferimento e la validazione, case-sensitive, non lo vede).
pub(crate) fn branch_already_exists(name: &str) -> GitError {
    GitError(format!("Branch '{name}' already exists."))
}

/// Errore per la cancellazione di un ramo locale che non esiste. Non è un
/// no-op silenzioso: il nome arriva da una selezione nella UI, se non
/// corrisponde a nulla lo stato mostrato è disallineato dal repository e
/// l'utente deve saperlo.
pub(crate) fn branch_not_found(name: &str) -> GitError {
    GitError(format!("Local branch '{name}' does not exist."))
}

/// Errore per la cancellazione del ramo su cui si è posizionati. La UI lo
/// impedisce già, ma il backend non si fida del chiamante: è l'ultimo punto
/// prima della scrittura su disco.
pub(crate) fn current_branch_not_deletable(name: &str) -> GitError {
    GitError(format!(
        "Branch '{name}' is the current branch and cannot be deleted."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn branch(name: &str, kind: BranchKind) -> Branch {
        Branch {
            name: name.to_string(),
            kind,
            current: false,
        }
    }

    #[test]
    fn accepts_a_plain_name_and_a_namespaced_one() {
        let existing = [branch("main", BranchKind::Local)];
        assert_eq!(validate_branch_name("develop", &existing), None);
        assert_eq!(validate_branch_name("feature/x", &existing), None);
    }

    #[test]
    fn rejects_malformed_names() {
        let existing: [Branch; 0] = [];
        for bad in [
            "",
            "two words",
            "-oops",
            "a..b",
            "a@{b",
            "tilde~",
            "trailing/",
            "trailing.",
            "trailing.lock",
        ] {
            assert!(
                validate_branch_name(bad, &existing).is_some(),
                "{bad:?} doveva essere rifiutato"
            );
        }
    }

    #[test]
    fn rejects_the_names_libgit2_would_silently_rewrite_or_reserve() {
        // Casi che `git branch` rifiuta ma libgit2 accetta normalizzandoli
        // (`a//b` -> `a/b`, `/lead` -> `lead`), più i riferimenti riservati:
        // senza questi controlli l'app creerebbe un ramo con un nome
        // diverso da quello digitato, o uno impossibile da usare.
        let existing: [Branch; 0] = [];
        for bad in [
            "a//b",
            "/lead",
            "trail/",
            ".hidden",
            "a/.b",
            "a/b.lock",
            "HEAD",
            "@",
            "refs/heads/x",
        ] {
            assert!(
                validate_branch_name(bad, &existing).is_some(),
                "{bad:?} doveva essere rifiutato"
            );
        }
    }

    #[test]
    fn rejects_a_name_that_collides_with_the_path_of_another_branch() {
        let existing = [
            branch("a", BranchKind::Local),
            branch("x/y", BranchKind::Local),
            branch("origin/main", BranchKind::Remote),
        ];

        assert_eq!(
            validate_branch_name("a/b", &existing),
            Some(branch_path_conflict("a/b", "a").0)
        );
        assert_eq!(
            validate_branch_name("x", &existing),
            Some(branch_path_conflict("x", "x/y").0)
        );
        // Un ramo remoto vive sotto `refs/remotes`: nessuna collisione.
        assert_eq!(validate_branch_name("origin", &existing), None);
    }

    #[test]
    fn a_clashing_name_names_the_kind_of_ref_it_collides_with() {
        let existing = [
            branch("develop", BranchKind::Local),
            branch("origin/main", BranchKind::Remote),
            branch("v0.1.0", BranchKind::Tag),
        ];

        assert_eq!(
            validate_branch_name("develop", &existing),
            Some(branch_already_exists("develop").0)
        );
        let remote = validate_branch_name("origin/main", &existing).unwrap();
        assert!(remote.contains("remote"), "messaggio poco chiaro: {remote}");
        let tag = validate_branch_name("v0.1.0", &existing).unwrap();
        assert!(tag.contains("tag"), "messaggio poco chiaro: {tag}");
    }
}
