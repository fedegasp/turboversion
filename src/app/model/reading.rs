//
// src/app/model/reading.rs
// TurboVersion
//
// 2026-11-11 Federico Gasperini
//

//! Un dato che si legge dal repository mentre si naviga, legato a ciò che
//! lo identifica: i file di un commit (chiave: l'id del commit) o il diff
//! di un file (chiave: file e contesto). La macchina a stati è una sola
//! perché le regole delicate sono le stesse per entrambi: quale esito si
//! accetta, quando un'attesa si chiude, che cosa vale la pena conservare.

use std::collections::VecDeque;

/// Ciò che un pannello mostra per la chiave `K`, e a che punto è la sua
/// lettura. Un valore letto per una chiave non può finire sotto un'altra:
/// chiave e stato stanno nello stesso valore, con i campi privati.
pub(crate) struct Reading<K, T> {
    key: K,
    load: Load<T>,
}

/// A che punto è una lettura. Ogni stato si disegna in modo diverso: un
/// pannello vuoto, da solo, non distinguerebbe "in arrivo", "illeggibile",
/// "nessuno potrà leggerlo" e "letto, ma vuoto".
pub(crate) enum Load<T> {
    /// Voluto, e non ancora arrivato.
    Awaiting,
    /// Arrivato.
    Loaded(T),
    /// Il backend non è riuscito a leggerlo; il messaggio è il suo.
    Failed(String),
    /// Nessuno lo leggerà: il thread che esegue le operazioni git non c'è
    /// più, prima che arrivasse o prima che venisse chiesto.
    Unavailable,
}

impl<K: PartialEq, T> Reading<K, T> {
    /// Una lettura appena voluta: in attesa finché non arriva, invece di
    /// mostrare il dato di prima sotto una chiave che non è la sua.
    pub(super) fn requested(key: K) -> Self {
        Self {
            key,
            load: Load::Awaiting,
        }
    }

    /// Una lettura che non si può più chiedere.
    pub(super) fn unavailable(key: K) -> Self {
        Self {
            key,
            load: Load::Unavailable,
        }
    }

    /// Una lettura voluta adesso: in attesa se git c'è ancora, altrimenti
    /// già "non disponibile", invece di aspettare un esito che non
    /// arriverà.
    pub(super) fn wanted(key: K, git_available: bool) -> Self {
        if git_available {
            Self::requested(key)
        } else {
            Self::unavailable(key)
        }
    }

    pub(crate) fn key(&self) -> &K {
        &self.key
    }

    pub(crate) fn load(&self) -> &Load<T> {
        &self.load
    }

    pub(super) fn load_mut(&mut self) -> &mut Load<T> {
        &mut self.load
    }

    pub(crate) fn is_awaiting(&self) -> bool {
        matches!(self.load, Load::Awaiting)
    }

    pub(super) fn is_loaded(&self) -> bool {
        matches!(self.load, Load::Loaded(_))
    }

    /// Registra l'esito della lettura per `key`, se è quello che questa
    /// vista sta aspettando; altrimenti lo scarta. Un esito per un'altra
    /// chiave è obsoleto (la selezione si è spostata prima che arrivasse);
    /// un secondo esito per la stessa chiave, già arrivata, riporterebbe in
    /// cima un cursore già mosso senza mostrare nulla di nuovo. In entrambi
    /// i casi la vista resta com'è, e così dopo `Failed` o `Unavailable`.
    ///
    /// L'identità della richiesta è la chiave e non un contatore. Per un
    /// commit il dato non cambia, quindi un esito per la chiave attesa è
    /// corretto qualunque delle richieste l'abbia prodotto.
    ///
    /// Per la working copy il dato cambia, e l'esito accettato può essere
    /// più vecchio del momento in cui la vista è entrata in attesa: tornando
    /// su un file la cui lettura è ancora in volo (A, B, di nuovo A) non se
    /// ne chiede un'altra, si accetta quella già partita.
    ///
    /// Ciò che si può garantire è che l'esito non sia più vecchio
    /// dell'ultima scrittura applicata, ma solo a una condizione, che è un
    /// obbligo per chi applica l'esito di una scrittura che cambia la
    /// working copy: deve chiamare `Model::forget_working_copy_diff`. Il
    /// worker è uno solo ed esegue i `Cmd` nell'ordine in cui arrivano,
    /// letture e scritture insieme, e gli esiti tornano nello stesso
    /// ordine; quando arriva l'esito di una scrittura, ogni lettura partita
    /// prima è quindi già tornata, e dimenticando il diff la lettura che lo
    /// sostituisce parte dopo, ed è eseguita dopo la scrittura. Lo
    /// rispettano gli esiti di stage, unstage e commit (vedi
    /// `results::apply_working_copy_written`), e `apply_loaded` alla
    /// rilettura del repository.
    pub(super) fn receive(&mut self, key: &K, result: Result<T, String>) {
        if !self.is_awaiting() || self.key != *key {
            return;
        }
        self.load = match result {
            Ok(value) => Load::Loaded(value),
            Err(message) => Load::Failed(message),
        };
    }

    /// Chiude un'attesa che nessun esito verrà più a chiudere. Una vista
    /// già arrivata, o già fallita, resta com'è: quello che mostra è ancora
    /// vero.
    pub(super) fn give_up_waiting(&mut self) {
        if self.is_awaiting() {
            self.load = Load::Unavailable;
        }
    }
}

/// Quante viste già lette si conservano per tipo. Abbastanza per tornare
/// indietro di qualche riga senza rileggere, e perché dopo la perdita del
/// worker restino leggibili gli ultimi commit e diff guardati, non solo
/// l'ultimo; poche, perché un diff può essere lungo.
pub(super) const KEPT_VIEWS: usize = 8;

/// Le ultime viste lasciate con il dato già arrivato, dalla più recente.
/// Serve solo a dati che non cambiano (i file e i diff di un commit):
/// ritrovarle evita di rileggerle, e dopo la perdita del worker sono ciò
/// che resta leggibile oltre alla vista a schermo.
///
/// Non è una cache del repository: si riempie solo di viste che l'utente
/// ha guardato fino all'arrivo del dato, e le letture saltate scorrendo in
/// fretta non arrivano mai fin qui.
pub(super) struct Kept<K, T> {
    views: VecDeque<Reading<K, T>>,
}

impl<K: PartialEq, T> Kept<K, T> {
    pub(super) fn new() -> Self {
        Self {
            views: VecDeque::with_capacity(KEPT_VIEWS),
        }
    }

    /// Conserva `view` se il suo dato è arrivato; una vista in attesa,
    /// fallita o non disponibile non ha nulla che valga la pena ritrovare,
    /// e tornarci la richiede. Oltre `KEPT_VIEWS` si perde la più vecchia.
    pub(super) fn keep(&mut self, view: Reading<K, T>) {
        if !view.is_loaded() {
            return;
        }
        self.views.retain(|v| v.key != view.key);
        self.views.push_front(view);
        self.views.truncate(KEPT_VIEWS);
    }

    /// Riprende la vista conservata per `key`, se c'è, togliendola da qui:
    /// torna a essere la vista a schermo.
    pub(super) fn take(&mut self, key: &K) -> Option<Reading<K, T>> {
        let index = self.views.iter().position(|v| v.key == *key)?;
        self.views.remove(index)
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.views.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loaded(key: u32) -> Reading<u32, &'static str> {
        let mut view = Reading::requested(key);
        view.receive(&key, Ok("data"));
        assert!(view.is_loaded());
        view
    }

    #[test]
    fn only_the_awaited_key_is_accepted_and_only_once() {
        let mut view: Reading<u32, &str> = Reading::requested(1);
        view.receive(&2, Ok("other"));
        assert!(view.is_awaiting(), "chiave diversa: obsoleto");

        view.receive(&1, Ok("first"));
        view.receive(&1, Ok("again"));
        assert!(matches!(view.load(), Load::Loaded("first")), "doppione");
    }

    #[test]
    fn an_error_or_a_lost_worker_closes_the_wait_for_good() {
        let mut failed: Reading<u32, &str> = Reading::requested(1);
        failed.receive(&1, Err("boom".to_string()));
        failed.receive(&1, Ok("late"));
        assert!(matches!(failed.load(), Load::Failed(m) if m == "boom"));

        let mut lost: Reading<u32, &str> = Reading::requested(1);
        lost.give_up_waiting();
        lost.receive(&1, Ok("late"));
        assert!(
            matches!(lost.load(), Load::Unavailable),
            "nessuna attesa da riaprire"
        );

        let mut arrived = loaded(1);
        arrived.give_up_waiting();
        assert!(arrived.is_loaded(), "ciò che è arrivato resta vero");

        assert!(matches!(
            Reading::<u32, &str>::wanted(1, false).load(),
            Load::Unavailable
        ));
        assert!(Reading::<u32, &str>::wanted(1, true).is_awaiting());
    }

    #[test]
    fn kept_holds_only_loaded_views_newest_first_up_to_its_capacity() {
        let mut kept = Kept::new();
        kept.keep(Reading::<u32, &str>::requested(100));
        let mut failed = Reading::requested(101);
        failed.receive(&101, Err("boom".to_string()));
        kept.keep(failed);
        assert_eq!(kept.len(), 0, "niente di arrivato da conservare");

        let extra = 3;
        for key in 0..(KEPT_VIEWS as u32 + extra) {
            kept.keep(loaded(key));
        }
        assert_eq!(kept.len(), KEPT_VIEWS);
        assert!(kept.take(&0).is_none(), "la più vecchia è uscita");
        assert!(kept.take(&(extra - 1)).is_none());
        assert!(kept.take(&extra).is_some(), "la più vecchia rimasta");
        assert!(kept.take(&extra).is_none(), "ripresa: non c'è più");

        // Conservare di nuovo la stessa chiave non la duplica.
        kept.keep(loaded(50));
        kept.keep(loaded(50));
        assert!(kept.take(&50).is_some());
        assert!(kept.take(&50).is_none());
    }
}
