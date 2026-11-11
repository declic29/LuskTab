//! Replies to agents (Lusk Tab): the `error` of a reply on the control channel, in the
//! interface language. A message the interface catalog knows is translated by it; the control
//! channel's own technical messages (screenshots, synthetic input, printing, the `ui.*`
//! commands) by the French templates below, where `{}` stands for a value. The reply is
//! translated as it leaves, so the code that builds and compares these messages keeps them in
//! English.

use serde_json::Value as Json;

use crate::i18n::{self, Language};

/// English template → French template; each `{}` matches a value and is copied, in order.
const FRENCH: &[(&str, &str)] = &[
    ("theme mode must be system, light, or dark", "le mode de thème doit être system, light ou dark"),
    ("missing `percent` ({}–{}, or \"auto\")", "`percent` manquant ({}–{}, ou \"auto\")"),
    ("unknown language {} (use one of {})", "langue inconnue {} (utilisez l'une de celles-ci : {})"),
    (
        "unknown File page {} (use home, new, open, info, saveAs, print, export, options or close)",
        "page du menu Fichier inconnue {} (utilisez home, new, open, info, saveAs, print, export, options ou close)",
    ),
    (
        "screenshot failed: the document changed while the capture was queued",
        "échec de la capture d'écran : le document a changé pendant que la capture attendait",
    ),
    ("screenshot failed: the capture belongs to another viewport", "échec de la capture d'écran : la capture appartient à une autre fenêtre"),
    (
        "screenshot failed: the window size overflows the screenshot budget",
        "échec de la capture d'écran : la taille de la fenêtre dépasse le budget des captures",
    ),
    (
        "screenshot failed: the window size overflows the screenshot encoding budget",
        "échec de la capture d'écran : la taille de la fenêtre dépasse le budget d'encodage des captures",
    ),
    (
        "screenshot encoding budget exceeded: {} bytes of transient working memory for one {}×{} capture, over the {}-byte encoding limit",
        "budget d'encodage de la capture dépassé : {} octets de mémoire de travail temporaire pour une capture de {}×{}, au-delà de la limite d'encodage de {} octets",
    ),
    (
        "screenshot base64 budget exceeded: the reply for one {}×{} capture could be {} bytes, over the {}-byte base64 limit; request a path instead",
        "budget base64 de la capture dépassé : la réponse pour une capture de {}×{} pourrait faire {} octets, au-delà de la limite base64 de {} octets ; demandez plutôt un chemin de fichier",
    ),
    (
        "screenshot base64 budget exceeded: the {}×{} reply would be {} bytes, over the {}-byte base64 limit; request a path instead",
        "budget base64 de la capture dépassé : la réponse de {}×{} ferait {} octets, au-delà de la limite base64 de {} octets ; demandez plutôt un chemin de fichier",
    ),
    (
        "screenshot peak-memory budget exceeded: {} bytes retained plus {} bytes encoding for one capture is over the {}-byte peak limit",
        "budget de mémoire maximale de la capture dépassé : {} octets conservés plus {} octets d'encodage pour une capture dépassent la limite maximale de {} octets",
    ),
    (
        "screenshot budget exceeded: {} bytes for one {}×{} image, over the {}-byte per-image limit",
        "budget des captures dépassé : {} octets pour une image de {}×{}, au-delà de la limite de {} octets par image",
    ),
    (
        "screenshot budget exceeded: {} bytes for this image would exceed the {}-byte aggregate limit",
        "budget des captures dépassé : {} octets pour cette image dépasseraient la limite totale de {} octets",
    ),
    ("screenshot dimensions overflow the aspect check", "les dimensions de la capture dépassent la vérification des proportions"),
    ("screenshot dimensions overflow", "les dimensions de la capture débordent"),
    ("screenshot has {} pixels for its {}×{} dimensions", "la capture a {} pixels pour des dimensions de {}×{}"),
    (
        "screenshot is {}×{} pixels, expected {}×{}: the window changed size while it was captured",
        "la capture fait {}×{} pixels au lieu de {}×{} : la fenêtre a changé de taille pendant la capture",
    ),
    (
        "screenshot is {}×{} pixels, expected {}×{}: the aspect ratio changed while it was captured",
        "la capture fait {}×{} pixels au lieu de {}×{} : les proportions ont changé pendant la capture",
    ),
    (
        "screenshot timeout: the window never reported a capturable size (it may be minimized or not laid out)",
        "délai de capture dépassé : la fenêtre n'a jamais indiqué de taille capturable (elle est peut-être réduite ou pas encore affichée)",
    ),
    ("no frame was presented before the screenshot timeout", "aucune image n'a été affichée avant la fin du délai de capture"),
    (
        "screenshot timeout: the image completed after its deadline; no file was written",
        "délai de capture dépassé : l'image a été terminée après l'échéance ; aucun fichier n'a été écrit",
    ),
    ("screenshot has no expected size", "la capture n'a pas de taille attendue"),
    (
        "screenshot geometry changed while it was captured; no image was written",
        "la géométrie a changé pendant la capture ; aucune image n'a été écrite",
    ),
    ("bad image", "image incorrecte"),
    ("screenshot base64 length overflow", "la longueur base64 de la capture déborde"),
    ("too many synthetic input events (limit {} queued at once)", "trop d'événements de saisie synthétique (limite : {} en attente à la fois)"),
    ("synthetic input exceeds the {}-byte queue limit", "la saisie synthétique dépasse la limite de file d'attente de {} octets"),
    (
        "text input is {} bytes, over the {}-byte limit for one event",
        "la saisie de texte fait {} octets, au-delà de la limite de {} octets par événement",
    ),
    ("print settings must be an object", "les réglages d'impression doivent être un objet"),
    (
        "unknown print setting; use printer, copies, duplex, grayscale or refresh",
        "réglage d'impression inconnu ; utilisez printer, copies, duplex, grayscale ou refresh",
    ),
    (
        "wait for the active print submission before changing settings",
        "attendez la fin de l'envoi d'impression en cours avant de changer les réglages",
    ),
    ("printer must be a name from the discovered list", "printer doit être un nom de la liste des imprimantes trouvées"),
    ("copies must be an integer from 1 to 999", "copies doit être un nombre entier de 1 à 999"),
    ("duplex must be printerDefault, off, longEdge or shortEdge", "duplex doit valoir printerDefault, off, longEdge ou shortEdge"),
    ("grayscale must be a boolean", "grayscale doit être un booléen"),
    ("refresh must be a boolean", "refresh doit être un booléen"),
    ("native printer discovery is unavailable here", "la recherche des imprimantes du système n'est pas disponible ici"),
    ("printer discovery or submission is already active", "une recherche d'imprimantes ou un envoi d'impression est déjà en cours"),
];

/// `text` matched against `template` (whose `{}` match any text): the matched values, in order.
fn captures<'a>(template: &str, text: &'a str) -> Option<Vec<&'a str>> {
    let pieces: Vec<&str> = template.split("{}").collect();
    let (first, rest) = pieces.split_first()?;
    let mut left = text.strip_prefix(first)?;
    let mut values = Vec::new();
    for (k, piece) in rest.iter().enumerate() {
        let last = k + 1 == rest.len();
        let at = if last {
            if piece.is_empty() { left.len() } else { left.len().checked_sub(piece.len()).filter(|at| left.get(*at..) == Some(*piece))? }
        } else {
            // A value never holds the next literal piece; an empty piece can't occur between
            // two values in the templates above.
            left.find(piece)?
        };
        values.push(left.get(..at)?);
        left = left.get(at + piece.len()..)?;
    }
    left.is_empty().then_some(values)
}

/// `message` in French: a template's translation with its values, or `None`.
fn french(message: &str) -> Option<String> {
    FRENCH.iter().find_map(|(en, fr)| {
        let values = captures(en, message)?;
        let mut out = String::new();
        let mut values = values.into_iter();
        for (k, piece) in fr.split("{}").enumerate() {
            if k > 0 {
                out.push_str(values.next().unwrap_or(""));
            }
            out.push_str(piece);
        }
        Some(out)
    })
}

/// An error message for an agent in `language`.
pub(crate) fn message(message: &str, language: Language) -> String {
    if language == Language::En {
        return message.to_string();
    }
    if language == Language::Fr
        && let Some(fr) = french(message)
    {
        return fr;
    }
    language.tr_error(message)
}

/// A reply with its `error` in the language of the last frame drawn on this thread.
pub(crate) fn reply(mut reply: Json) -> Json {
    let language = i18n::last_language();
    if language != Language::En
        && let Some(error) = reply.get("error").and_then(Json::as_str).map(str::to_string)
    {
        reply["error"] = Json::String(message(&error, language));
    }
    reply
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn french_templates_keep_their_values() {
        let fr = |m: &str| message(m, Language::Fr);
        assert_eq!(fr("copies must be an integer from 1 to 999"), "copies doit être un nombre entier de 1 à 999");
        assert_eq!(fr("unknown language \"xx\" (use one of en, fr)"), "langue inconnue \"xx\" (utilisez l'une de celles-ci : en, fr)");
        assert_eq!(
            fr("screenshot is 800×600 pixels, expected 1024×768: the window changed size while it was captured"),
            "la capture fait 800×600 pixels au lieu de 1024×768 : la fenêtre a changé de taille pendant la capture"
        );
        assert_eq!(
            fr("text input is 70000 bytes, over the 65536-byte limit for one event"),
            "la saisie de texte fait 70000 octets, au-delà de la limite de 65536 octets par événement"
        );
        // A message the interface catalog knows.
        assert_eq!(fr("Select a chart first."), "Sélectionnez d'abord un graphique.");
        // English stays English, and an unknown message stays as it is.
        assert_eq!(message("bad image", Language::En), "bad image");
        assert_eq!(fr("something unexpected"), "something unexpected");
    }

    #[test]
    fn replies_follow_the_last_frame_language() {
        let r = reply(serde_json::json!({"ok": false, "error": "bad image"}));
        assert_eq!(r["error"], "bad image", "English until a frame sets another language");
    }
}
