//
// src/app/list_panel.rs
// TurboVersion
//
// 2026-11-04 Federico Gasperini
//

//! `ListPanel<T>`: gli elementi di una lista e il cursore che la percorre,
//! tenuti insieme in un unico valore.
//!
//! Prima erano due campi distinti del `Model` per ogni pannello (un
//! `Vec<T>` e un `ListState`), coerenti solo finché tutti i punti che li
//! toccavano si ricordavano di aggiornarli insieme; abbinare gli elementi
//! di una lista al cursore di un'altra era un semplice refuso. Qui i due
//! campi sono privati e si modificano attraverso i metodi, che mantengono
//! l'invariante:
//!
//! > la selezione è `None` se e solo se la lista è vuota; altrimenti è
//! > sempre un indice valido.
//!
//! Il tipo vive dentro `app` perché è lì che si trova chi ha il diritto di
//! modificarlo: tutto ciò che cambia la lista o la selezione è
//! `pub(in crate::app)`, mentre a `ui` restano la lettura e `parts_mut`,
//! l'unica via d'uscita del `ListState` (che ratatui aggiorna mentre
//! disegna). "Solo `update` muta il Model" non è quindi una disciplina da
//! ricordare: un pannello che chiamasse `set`, `retain` o `push_selected`
//! non compila. Una visibilità `pub(in crate::app)` è legale solo se il
//! modulo sta dentro `crate::app`, da cui la collocazione.
//!
//! Il tipo non sta in `ui/` perché lo usano sia `update` sia `view`: il
//! `ListState` è lo stato di selezione *e* di scorrimento che ratatui
//! aggiorna durante il rendering, e che deve sopravvivere da un frame al
//! successivo.

use ratatui::widgets::ListState;

pub(crate) struct ListPanel<T> {
    items: Vec<T>,
    state: ListState,
}

impl<T> ListPanel<T> {
    /// Costruisce la lista con il cursore sulla prima riga (su nessuna
    /// riga, se `items` è vuoto).
    ///
    /// È un costruttore inerente e non un `impl From<Vec<T>>` perché un
    /// trait pubblico non si può restringere: `From` e `Default`
    /// resterebbero raggiungibili da `ui`, e chi avesse in mano un
    /// `&mut ListPanel` potrebbe svuotare la lista con un `*list =
    /// ListPanel::default()` aggirando i metodi ristretti qui sotto.
    pub(in crate::app) fn new(items: Vec<T>) -> Self {
        // Un `Some(0)` incondizionato indicherebbe una riga inesistente, e
        // chi usasse quell'indice per raggiungere l'elemento finirebbe
        // fuori dai limiti (una lista vuota è normale: un repository senza
        // modifiche o senza rami remoti).
        let selected = (!items.is_empty()).then_some(0);
        Self {
            items,
            state: ListState::default().with_selected(selected),
        }
    }

    /// Una lista vuota, senza selezione: lo stato di partenza di ogni
    /// pannello, prima che il repository venga letto.
    pub(in crate::app) fn empty() -> Self {
        Self::new(Vec::new())
    }

    /// Sostituisce il contenuto e riporta il cursore sulla prima riga (su
    /// nessuna riga, se la lista arriva vuota): una lista più corta non può
    /// quindi lasciare la selezione fuori dai limiti.
    pub(in crate::app) fn set(&mut self, items: Vec<T>) {
        *self = Self::new(items);
    }

    pub(crate) fn items(&self) -> &[T] {
        &self.items
    }

    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Indice della riga selezionata. Per l'invariante è `Some` esattamente
    /// quando la lista non è vuota.
    pub(crate) fn selected_index(&self) -> Option<usize> {
        self.state.selected()
    }

    pub(crate) fn selected(&self) -> Option<&T> {
        self.selected_index().and_then(|i| self.items().get(i))
    }

    pub(in crate::app) fn selected_mut(&mut self) -> Option<&mut T> {
        let i = self.selected_index()?;
        self.items.get_mut(i)
    }

    /// Elementi e cursore insieme, come li vuole `render_stateful_widget`.
    /// Un `items()` seguito da un `state_mut()` non compilerebbe (prestito
    /// condiviso e prestito esclusivo dello stesso valore); qui i due campi
    /// vengono prestati separatamente.
    ///
    /// È l'unica via d'uscita del `ListState`, e porta al solo rendering:
    /// ratatui si limita a riclampare la selezione prima di disegnare
    /// (nessuna selezione su lista vuota, altrimenti al più l'ultima riga),
    /// cioè ristabilisce la stessa invariante mantenuta qui. Chi ottenesse
    /// questo prestito per altri scopi potrebbe invece spostare il cursore
    /// fuori dai limiti.
    pub(crate) fn parts_mut(&mut self) -> (&[T], &mut ListState) {
        (&self.items, &mut self.state)
    }

    /// Muove il cursore di `delta` righe, fermandosi ai bordi. Su una lista
    /// vuota non c'è nulla da muovere.
    pub(in crate::app) fn step(&mut self, delta: i32) {
        if self.is_empty() {
            return;
        }
        let cur = self.selected_index().unwrap_or(0) as i32;
        let last = self.len() as i32 - 1;
        self.state
            .select(Some((cur + delta).clamp(0, last) as usize));
    }

    /// Aggiunge un elemento in coda e lo seleziona.
    pub(in crate::app) fn push_selected(&mut self, item: T) {
        self.items.push(item);
        self.state.select(Some(self.len() - 1));
    }

    /// Tiene solo gli elementi che soddisfano `keep`, riportando il cursore
    /// nei limiti della lista rimasta.
    pub(in crate::app) fn retain(&mut self, keep: impl FnMut(&T) -> bool) {
        self.items.retain(keep);
        let last = self.len().checked_sub(1);
        let selected = self.selected_index().zip(last).map(|(s, l)| s.min(l));
        self.state.select(selected);
    }

    /// Porta il cursore sul primo elemento che soddisfa `wanted`, e dice se
    /// l'ha trovato; se non c'è, il cursore resta dov'era. Serve a ritrovare
    /// una riga per identità dopo che la lista è stata sostituita.
    pub(in crate::app) fn select_first(&mut self, wanted: impl FnMut(&T) -> bool) -> bool {
        match self.items.iter().position(wanted) {
            Some(index) => {
                self.state.select(Some(index));
                true
            }
            None => false,
        }
    }

    /// Porta il cursore su `index`, limitato all'ultima riga esistente; su
    /// una lista vuota non seleziona nulla.
    pub(in crate::app) fn select_nearest(&mut self, index: usize) {
        let last = self.len().checked_sub(1);
        self.state.select(last.map(|l| index.min(l)));
    }

    /// Come `select_nearest`, per i test che posizionano la selezione senza
    /// passare da `update`, compresi quelli di rendering in `ui`: è l'unico
    /// metodo che muta e resta `pub(crate)`, ma essendo compilato solo nei
    /// test non allenta il vincolo sul codice vero.
    #[cfg(test)]
    pub(crate) fn select(&mut self, index: usize) {
        self.select_nearest(index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel(n: usize) -> ListPanel<usize> {
        ListPanel::new((0..n).collect::<Vec<_>>())
    }

    #[test]
    fn an_empty_list_has_no_selection_and_a_filled_one_starts_at_the_top() {
        let empty = panel(0);
        assert_eq!(empty.selected_index(), None);
        assert_eq!(empty.selected(), None);
        assert!(empty.is_empty());

        let filled = panel(3);
        assert_eq!(filled.selected_index(), Some(0));
        assert_eq!(filled.selected(), Some(&0));
        assert_eq!(filled.len(), 3);
    }

    #[test]
    fn set_with_a_shorter_list_brings_the_selection_back_in_range() {
        let mut p = panel(5);
        p.step(4);
        assert_eq!(p.selected_index(), Some(4));

        p.set(vec![7, 8]);
        assert_eq!(p.selected_index(), Some(0));
        assert_eq!(p.selected(), Some(&7));

        p.set(Vec::new());
        assert_eq!(p.selected_index(), None, "lista vuota, nessuna selezione");
    }

    #[test]
    fn step_clamps_at_both_ends_and_tolerates_an_empty_list() {
        let mut p = panel(3);
        p.step(-1);
        assert_eq!(p.selected_index(), Some(0));
        p.step(10);
        assert_eq!(p.selected_index(), Some(2));
        p.step(1);
        assert_eq!(p.selected_index(), Some(2));

        let mut empty = panel(0);
        empty.step(1); // niente panico, niente selezione inventata
        assert_eq!(empty.selected_index(), None);
    }

    #[test]
    fn push_selected_appends_and_moves_the_cursor_onto_the_new_item() {
        let mut p = panel(0);
        p.push_selected(42);
        assert_eq!(p.items(), [42]);
        assert_eq!(p.selected(), Some(&42));

        p.push_selected(43);
        assert_eq!(p.selected_index(), Some(1));
    }

    #[test]
    fn retain_keeps_the_selection_inside_the_shrunken_list() {
        let mut p = panel(4);
        p.step(3);
        assert_eq!(p.selected_index(), Some(3));

        p.retain(|&x| x != 3);
        assert_eq!(p.items(), [0, 1, 2]);
        assert_eq!(p.selected_index(), Some(2), "era l'ultima riga");

        p.retain(|_| false);
        assert!(p.is_empty());
        assert_eq!(p.selected_index(), None);
    }

    #[test]
    fn selected_mut_edits_the_selected_item_only() {
        let mut p = panel(3);
        p.step(1);
        if let Some(x) = p.selected_mut() {
            *x = 99;
        }
        assert_eq!(p.items(), [0, 99, 2]);
    }

    #[test]
    fn select_first_finds_an_item_or_leaves_the_cursor_alone() {
        let mut p = panel(4);
        assert!(p.select_first(|&x| x == 2));
        assert_eq!(p.selected_index(), Some(2));

        assert!(!p.select_first(|&x| x == 99));
        assert_eq!(p.selected_index(), Some(2), "non trovato: resta dov'era");

        let mut empty = panel(0);
        assert!(!empty.select_first(|_| true));
        assert_eq!(empty.selected_index(), None);
    }

    #[test]
    fn select_clamps_to_the_last_row_and_does_nothing_on_an_empty_list() {
        let mut p = panel(3);
        p.select(1);
        assert_eq!(p.selected_index(), Some(1));
        p.select(99);
        assert_eq!(p.selected_index(), Some(2));

        let mut empty = panel(0);
        empty.select(0);
        assert_eq!(empty.selected_index(), None);
    }
}
