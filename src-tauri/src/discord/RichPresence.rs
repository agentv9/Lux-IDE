use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::mpsc, thread, time::Duration};
use tauri::Listener;

const APPLICATION_ID: &str = "1566318110493909032";
const ICON_BASE: &str =
    "https://raw.githubusercontent.com/iCrawl/discord-vscode/master/assets/icons";

#[derive(Clone, Deserialize)]
struct ActiveFile {
    path: String,
    language: String,
}

#[derive(Deserialize)]
struct Language {
    language: String,
    image: String,
}

#[derive(Deserialize)]
struct Image {
    image: String,
}

#[derive(Deserialize)]
struct Definitions {
    #[serde(rename = "KNOWN_LANGUAGES")]
    languages: Vec<Language>,
    #[serde(rename = "KNOWN_EXTENSIONS")]
    extensions: BTreeMap<String, Image>,
}

impl Definitions {
    fn image(&self, filename: &str, language: &str) -> &str {
        let filename = filename.to_lowercase();
        if let Some(image) = self.extensions.get(&filename) {
            return &image.image;
        }
        for (pattern, image) in &self.extensions {
            if let Some(pattern) = pattern.strip_prefix('/').and_then(|p| p.strip_suffix("/i")) {
                if regex::RegexBuilder::new(pattern)
                    .case_insensitive(true)
                    .build()
                    .is_ok_and(|regex| regex.is_match(&filename))
                {
                    return &image.image;
                }
            }
        }
        // Prefer compound extensions such as .test.ts over .ts.
        if let Some((_, image)) = self
            .extensions
            .iter()
            .filter(|(key, _)| !key.starts_with('/') && filename.ends_with(key.as_str()))
            .max_by_key(|(key, _)| key.len())
        {
            return &image.image;
        }
        self.languages
            .iter()
            .find(|entry| entry.language.eq_ignore_ascii_case(language))
            .map(|entry| entry.image.as_str())
            .unwrap_or("text")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_bundled_icons_with_language_and_plaintext_fallbacks() {
        let definitions: Definitions =
            serde_json::from_str(include_str!("../../data/languages.json")).unwrap();
        assert_eq!(definitions.image("package.json", "json"), "npm");
        assert_eq!(
            definitions.image("vite.config.ts", "typescript"),
            "viteconfig"
        );
        assert_eq!(definitions.image("main.rs", "rust"), "rust");
        assert_eq!(definitions.image("Component.vue", "plaintext"), "vue");
        assert_eq!(definitions.image("untitled", "python"), "python");
        assert_eq!(definitions.image("unknown", "unknown"), "text");
    }
}

pub fn init(app: &tauri::AppHandle) {
    let definitions: Definitions = serde_json::from_str(include_str!("../../data/languages.json"))
        .expect("invalid bundled language definitions");
    let (sender, receiver) = mpsc::channel();
    app.listen("editor-active-file-changed", move |event| {
        if let Ok(file) = serde_json::from_str::<Option<ActiveFile>>(event.payload()) {
            let _ = sender.send(file);
        }
    });

    // IPC and reconnects stay off the UI thread. Keep the latest file if Discord starts later.
    thread::spawn(move || {
        let mut client = DiscordIpcClient::new(APPLICATION_ID);
        let mut connected = false;
        let mut current: Option<ActiveFile> = None;
        loop {
            match receiver.recv_timeout(Duration::from_secs(15)) {
                Ok(file) => current = file,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            while let Ok(file) = receiver.try_recv() {
                current = file;
            }
            if !connected {
                connected = client.connect().is_ok();
                if !connected {
                    continue;
                }
            }
            let result = if let Some(file) = &current {
                let filename = file.path.rsplit(['/', '\\']).next().unwrap_or(&file.path);
                let image = definitions.image(filename, &file.language);
                let image_url = format!("{ICON_BASE}/{image}.png");
                let details: String = format!("Editing {filename}").chars().take(128).collect();
                client.set_activity(
                    activity::Activity::new()
                        .details(&details)
                        .state("Working in Lux")
                        .assets(
                            activity::Assets::new()
                                .large_image(&image_url)
                                .large_text(image)
                                .small_image("lux_logo_master")
                                .small_text("Lux IDE"),
                        ),
                )
            } else {
                client.clear_activity()
            };
            if result.is_err() {
                let _ = client.close();
                connected = false;
            }
        }
        let _ = client.close();
    });
}
