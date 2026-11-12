//! Lusk Tab (repris de Lusk Texte) : les espaces en ligne de cet ordinateur, OneDrive et Google
//! Drive. Lusk Tab ne se connecte à aucun compte : il enregistre dans le dossier que
//! l'application du service synchronise (OneDrive, fourni avec Windows ; « Google Drive pour
//! ordinateur »), et c'est elle qui envoie le classeur en ligne.

use std::path::{Path, PathBuf};

/// Un dossier en ligne : son nom affiché (« OneDrive », « OneDrive - Entreprise »,
/// « Google Drive ») et son chemin.
pub type CloudDir = (String, PathBuf);

/// Les dossiers OneDrive synchronisés sur cet ordinateur. Windows les indique dans les variables
/// OneDriveConsumer (compte personnel), OneDriveCommercial (compte professionnel) et OneDrive ;
/// ailleurs (Linux avec un client OneDrive), on regarde ~/OneDrive. Vide quand OneDrive n'est pas
/// installé.
pub fn onedrive_dirs() -> Vec<CloudDir> {
    let vars: Vec<PathBuf> = ["OneDriveConsumer", "OneDriveCommercial", "OneDrive"]
        .iter()
        .filter_map(std::env::var_os)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .collect();
    onedrive_dirs_from(&vars, home().as_deref())
}

/// [`onedrive_dirs`] à partir des dossiers que donnent les variables et du dossier personnel.
fn onedrive_dirs_from(vars: &[PathBuf], home: Option<&Path>) -> Vec<CloudDir> {
    let mut found: Vec<CloudDir> = Vec::new();
    for dir in vars.iter().cloned().chain(home.map(|h| h.join("OneDrive"))) {
        if !dir.is_dir() {
            continue;
        }
        let key = dir.canonicalize().unwrap_or_else(|_| dir.clone());
        if found.iter().any(|(_, d)| d.canonicalize().unwrap_or_else(|_| d.clone()) == key) {
            continue;
        }
        let label = dir.file_name().map_or_else(|| "OneDrive".to_string(), |n| n.to_string_lossy().into_owned());
        found.push((label, dir));
    }
    found
}

/// Les dossiers « Mon Drive » de l'application Google Drive pour ordinateur. Windows : le lecteur
/// que l'application crée (G: par défaut, n'importe quelle lettre de D à Z) ou le dossier en
/// miroir sous le dossier personnel ; macOS : ~/Library/CloudStorage/GoogleDrive-<compte> ; Linux
/// (pas de client officiel) : ~/GoogleDrive ou ~/google-drive. Vide quand Google Drive n'est pas
/// installé.
pub fn google_drive_dirs() -> Vec<CloudDir> {
    let drives: Vec<PathBuf> = if cfg!(windows) { ('D'..='Z').map(|l| PathBuf::from(format!("{l}:\\"))).collect() } else { Vec::new() };
    google_drive_dirs_from(&drives, home().as_deref())
}

/// [`google_drive_dirs`] à partir des racines de lecteurs à essayer et du dossier personnel.
fn google_drive_dirs_from(drives: &[PathBuf], home: Option<&Path>) -> Vec<CloudDir> {
    const MY_DRIVE: [&str; 2] = ["Mon Drive", "My Drive"];
    let mut candidates: Vec<CloudDir> = Vec::new();
    for root in drives {
        for name in MY_DRIVE {
            candidates.push(("Google Drive".into(), root.join(name)));
        }
    }
    if let Some(h) = home {
        for name in MY_DRIVE {
            candidates.push(("Google Drive".into(), h.join(name)));
        }
        if let Ok(entries) = std::fs::read_dir(h.join("Library").join("CloudStorage")) {
            for e in entries.flatten() {
                let n = e.file_name().to_string_lossy().into_owned();
                if let Some(account) = n.strip_prefix("GoogleDrive-") {
                    for name in MY_DRIVE {
                        candidates.push((format!("Google Drive ({account})"), e.path().join(name)));
                    }
                }
            }
        }
        for name in ["GoogleDrive", "google-drive", "Google Drive"] {
            candidates.push(("Google Drive".into(), h.join(name)));
        }
    }
    let mut found: Vec<CloudDir> = Vec::new();
    for (label, dir) in candidates {
        if dir.is_dir() && !found.iter().any(|(_, d)| d == &dir) {
            found.push((label, dir));
        }
    }
    found
}

/// Le dossier personnel.
fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).filter(|h| !h.is_empty()).map(PathBuf::from)
}

/// Les dossiers OneDrive et Google Drive de cet ordinateur.
pub type CloudDirs = (Vec<CloudDir>, Vec<CloudDir>);

/// [`CloudDirs`], cherchés au plus une fois toutes les 10 secondes (la recherche de Google Drive
/// essaie chaque lettre de lecteur).
pub fn cloud_dirs(ctx: &egui::Context) -> CloudDirs {
    let id = egui::Id::unique("lusk_cloud_dirs");
    let now = ctx.input(|i| i.time);
    if let Some((at, dirs)) = ctx.data(|d| d.get_temp::<(f64, CloudDirs)>(id))
        && now - at < 10.0
    {
        return dirs;
    }
    let dirs = if cfg!(target_arch = "wasm32") { (Vec::new(), Vec::new()) } else { (onedrive_dirs(), google_drive_dirs()) };
    ctx.data_mut(|d| d.insert_temp(id, (now, dirs.clone())));
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lusk-cloud-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn onedrive_folders_from_the_variables_and_home() {
        let root = temp("onedrive");
        let personal = root.join("OneDrive");
        let work = root.join("OneDrive - Lusk");
        std::fs::create_dir_all(&personal).unwrap();
        std::fs::create_dir_all(&work).unwrap();
        // The same folder from a variable and from home counts once; a missing folder not at all.
        let found = onedrive_dirs_from(&[personal.clone(), work.clone(), root.join("absent")], Some(&root));
        assert_eq!(found, vec![("OneDrive".to_string(), personal), ("OneDrive - Lusk".to_string(), work)]);
        assert!(onedrive_dirs_from(&[], Some(&root.join("nobody"))).is_empty(), "no OneDrive: no folder");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn google_drive_my_drive_on_a_drive_letter_or_in_home() {
        let root = temp("google");
        let drive = root.join("G");
        std::fs::create_dir_all(drive.join("Mon Drive")).unwrap();
        let found = google_drive_dirs_from(&[root.join("F"), drive.clone()], Some(&root.join("home")));
        assert_eq!(found, vec![("Google Drive".to_string(), drive.join("Mon Drive"))]);
        assert!(google_drive_dirs_from(&[], None).is_empty(), "not installed: no folder");
        let _ = std::fs::remove_dir_all(&root);
    }
}
