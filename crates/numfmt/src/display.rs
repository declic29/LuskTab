//! How cells are *shown* (Lusk Tab): the region whose separators, month and day names and system
//! dates the display uses, and the words for TRUE, FALSE and the error values.
//!
//! Only what a person reads changes. Stored values, the format codes saved in files, calculation
//! results and `TEXT()` keep their canonical en-US spelling: `TEXT()` runs under [`canonical`].
//!
//! The app sets the region once from the operating system's regional settings
//! ([`set_display_region`]) and the words from the interface language ([`set_french_words`]).
//! Tests and other single-thread callers can use [`with_display_region`], which only affects the
//! calling thread. Nothing is set by default, so the behaviour is GridCraft's original en-US one.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use gridcraft_core::{CellError, Region, region};

/// Index in `region::all()`; `usize::MAX` is the default region (en-US).
static REGION: AtomicUsize = AtomicUsize::new(usize::MAX);
static FRENCH_WORDS: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// Set while formatting a canonical result (`TEXT()`): the region is en-US.
    static CANONICAL: Cell<bool> = const { Cell::new(false) };
    /// A per-thread region that wins over the process-wide one.
    static OVERRIDE: Cell<Option<&'static Region>> = const { Cell::new(None) };
    /// Per-thread French words, winning over the process-wide setting.
    static WORDS_OVERRIDE: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Shows numbers and dates the way `region` writes them, in the whole process.
pub fn set_display_region(r: &'static Region) {
    let i = region::all().iter().position(|x| x.tag == r.tag).unwrap_or(usize::MAX);
    REGION.store(i, Ordering::Relaxed);
}

/// Shows TRUE, FALSE and the error values in French (VRAI, FAUX, #VALEUR!…), in the whole process.
pub fn set_french_words(on: bool) {
    FRENCH_WORDS.store(on, Ordering::Relaxed);
}

/// The region cells are shown in on this thread.
pub fn display_region() -> &'static Region {
    if CANONICAL.with(Cell::get) {
        return region::default_region();
    }
    if let Some(r) = OVERRIDE.with(Cell::get) {
        return r;
    }
    region::all().get(REGION.load(Ordering::Relaxed)).copied().unwrap_or_else(region::default_region)
}

/// Runs `f` with cells shown in `r` on this thread only.
pub fn with_display_region<T>(r: &'static Region, f: impl FnOnce() -> T) -> T {
    let previous = OVERRIDE.with(|c| c.replace(Some(r)));
    let out = f();
    OVERRIDE.with(|c| c.set(previous));
    out
}

/// Runs `f` with the French words on or off on this thread only.
pub fn with_french_words<T>(on: bool, f: impl FnOnce() -> T) -> T {
    let previous = WORDS_OVERRIDE.with(|c| c.replace(Some(on)));
    let out = f();
    WORDS_OVERRIDE.with(|c| c.set(previous));
    out
}

/// Runs `f` with the canonical en-US region (for results that are stored or calculated on).
pub(crate) fn canonical<T>(f: impl FnOnce() -> T) -> T {
    let previous = CANONICAL.with(|c| c.replace(true));
    let out = f();
    CANONICAL.with(|c| c.set(previous));
    out
}

fn french() -> bool {
    !CANONICAL.with(Cell::get) && WORDS_OVERRIDE.with(Cell::get).unwrap_or_else(|| FRENCH_WORDS.load(Ordering::Relaxed))
}

/// Are cells shown with French words on this thread (VRAI, #VALEUR!, and the labels reports
/// write)? Off while a canonical result is formatted.
pub fn french_words() -> bool {
    french()
}

/// TRUE or FALSE as shown.
pub fn bool_text(b: bool) -> &'static str {
    match (french(), b) {
        (true, true) => "VRAI",
        (true, false) => "FAUX",
        (false, true) => "TRUE",
        (false, false) => "FALSE",
    }
}

/// An error value as shown. In French, the names Excel in French uses; the errors it has no
/// French name for keep their English one.
pub fn error_text(e: CellError) -> &'static str {
    if !french() {
        return e.as_str();
    }
    match e {
        CellError::Null => "#NUL!",
        CellError::Value => "#VALEUR!",
        CellError::Name => "#NOM?",
        CellError::Num => "#NOMBRE!",
        CellError::Spill => "#PROPAGATION!",
        other => other.as_str(),
    }
}

/// The display region's system short date (built-in format 14) for the canonical `m/d/yyyy`, and
/// its date and time (built-in format 22) for `m/d/yyyy h:mm`; `None` for any other code.
pub(crate) fn system_date_code(code: &str) -> Option<&'static str> {
    let r = display_region();
    if r.tag == region::default_region().tag {
        return None;
    }
    match code {
        "m/d/yyyy" => Some(r.short_date),
        "m/d/yyyy h:mm" => Some(r.datetime),
        _ => None,
    }
}

/// The digit group separator as shown.
pub(crate) fn group() -> char {
    display_region().group
}

/// The decimal separator as shown.
pub(crate) fn decimal() -> char {
    display_region().decimal
}
