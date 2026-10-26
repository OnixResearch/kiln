use serde::Deserialize;

pub const MAX_ENTRIES: usize = 128;
const MAX_ID_BYTES: usize = 128;
const MAX_LABEL_BYTES: usize = 1024;
// Enough for every admitted string escaped as six-byte JSON Unicode sequences.
const MAX_JSON_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub project: String,
    pub history: String,
    pub project_label: String,
    pub history_label: String,
    pub source: Option<String>,
    pub revision: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Identity {
    pub project: String,
    pub history: String,
}

#[derive(Clone, Copy)]
pub enum Movement {
    Previous,
    Next,
    First,
    Last,
}

pub struct Model {
    pub entries: Vec<Entry>,
    pub active: Identity,
    pub cursor: Option<usize>,
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

impl Model {
    pub fn parse(json: &str, project: &str, history: &str) -> Result<Self, String> {
        if json.len() > MAX_JSON_BYTES {
            return Err("Index metadata exceeds the JSON byte limit".into());
        }
        let entries: Vec<Entry> = serde_json::from_str(json)
            .map_err(|error| format!("Invalid index metadata: {error}"))?;
        if entries.len() > MAX_ENTRIES {
            return Err(format!("Index exceeds {MAX_ENTRIES} histories"));
        }
        for (index, entry) in entries.iter().enumerate() {
            if !valid_id(&entry.project) || !valid_id(&entry.history) {
                return Err(
                    "Index identities must be 1–128 ASCII letters, digits, '.', '_' or '-'".into(),
                );
            }
            if [
                Some(&entry.project_label),
                Some(&entry.history_label),
                entry.source.as_ref(),
                entry.revision.as_ref(),
            ]
            .into_iter()
            .flatten()
            .any(|value| value.len() > MAX_LABEL_BYTES)
            {
                return Err("Index labels, source and revision must be at most 1024 bytes".into());
            }
            if entries[..index]
                .iter()
                .any(|other| other.project == entry.project && other.history == entry.history)
            {
                return Err("Index contains a duplicate repository/history identity".into());
            }
        }
        let mut model = Self {
            entries,
            active: Identity {
                project: String::new(),
                history: String::new(),
            },
            cursor: None,
        };
        model.select_external(project, history);
        Ok(model)
    }

    pub fn active_index(&self) -> Option<usize> {
        self.entries.iter().position(|entry| {
            entry.project == self.active.project && entry.history == self.active.history
        })
    }

    // r[impl kiln.dashboard.rats.behavior.scenario.restore]
    pub fn select_external(&mut self, project: &str, history: &str) {
        self.active.project.clear();
        self.active.project.push_str(project);
        self.active.history.clear();
        self.active.history.push_str(history);
        self.cursor = self.active_index();
    }

    pub fn move_cursor(&mut self, movement: Movement) {
        let Some(last) = self.entries.len().checked_sub(1) else {
            return;
        };
        self.cursor = Some(match movement {
            Movement::First => 0,
            Movement::Last => last,
            Movement::Next => self.cursor.map_or(0, |index| (index + 1).min(last)),
            Movement::Previous => self.cursor.map_or(last, |index| index.saturating_sub(1)),
        });
    }

    pub fn point(&mut self, index: usize) {
        if index < self.entries.len() {
            self.cursor = Some(index);
        }
    }

    pub fn candidate(&self) -> Option<&Entry> {
        self.cursor.and_then(|index| self.entries.get(index))
    }

    // The pristine widget starts each render at offset zero, with one header row
    // and unit-height data rows. Its selected row scrolls into the last visible slot.
    pub fn visible_start(&self, data_height: usize) -> usize {
        self.cursor
            .unwrap_or(0)
            .saturating_add(1)
            .saturating_sub(data_height.max(1))
    }
}

// Upstream DataTable truncates byte slices. Keep its cells ASCII so arbitrary
// admitted UTF-8 cannot panic; the browser detail uses the full original labels.
pub fn table_text(value: &str) -> String {
    value.chars().flat_map(char::escape_default).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(project: &str, history: &str) -> serde_json::Value {
        serde_json::json!({"project":project,"history":history,"project_label":"Répertoire 🐀","history_label":"Main","source":null,"revision":null})
    }

    // r[verify kiln.dashboard.rats.widget.scenario.selection]
    // r[verify kiln.dashboard.rats.behavior]
    // r[verify kiln.dashboard.rats.behavior.scenario.restore]
    #[test]
    fn exact_pair_and_missing_selection_require_explicit_candidate() {
        let json = serde_json::to_string(&vec![entry("a", "main"), entry("b", "main")]).unwrap();
        let mut model = Model::parse(&json, "b", "main").unwrap();
        assert_eq!(model.candidate().unwrap().project, "b");
        model.select_external("a", "missing");
        assert!(model.candidate().is_none());
        assert_eq!(model.active.history, "missing");
        model.move_cursor(Movement::Next);
        assert_eq!(model.candidate().unwrap().project, "a");
        assert!(model.active_index().is_none());
        model.select_external("b", "main");
        assert_eq!(model.cursor, Some(1));
        model.point(5);
        assert_eq!(model.cursor, Some(1));
        model.select_external("", "任意");
        assert!(model.candidate().is_none());
        assert_eq!(model.active.history, "任意");
    }

    // r[verify kiln.dashboard.rats.widget]
    #[test]
    fn bounded_metadata_accepts_limits_and_rejects_overflow_and_duplicates() {
        let rows: Vec<_> = (0..MAX_ENTRIES)
            .map(|index| entry("p", &index.to_string()))
            .collect();
        assert!(Model::parse(&serde_json::to_string(&rows).unwrap(), "p", "0").is_ok());
        let mut overflow = rows.clone();
        overflow.push(entry("p", "overflow"));
        assert!(Model::parse(&serde_json::to_string(&overflow).unwrap(), "p", "0").is_err());
        let mut valid = entry(&"x".repeat(MAX_ID_BYTES), "h");
        valid["project_label"] = "é".repeat(MAX_LABEL_BYTES / 2).into();
        let json = serde_json::to_string(&vec![&valid]).unwrap();
        assert!(Model::parse(&json, "p", "h").is_ok());
        valid["project_label"] = "é".repeat(MAX_LABEL_BYTES / 2 + 1).into();
        assert!(Model::parse(&serde_json::to_string(&vec![valid]).unwrap(), "p", "h").is_err());
        let punctuation = serde_json::to_string(&vec![entry("repo._-1", "history-2")]).unwrap();
        assert!(Model::parse(&punctuation, "repo._-1", "history-2").is_ok());
        for bad in [
            entry("", "h"),
            entry("é", "h"),
            entry("a/b", "h"),
            entry("p", "h\n"),
            entry(&"x".repeat(MAX_ID_BYTES + 1), "h"),
        ] {
            assert!(Model::parse(&serde_json::to_string(&vec![bad]).unwrap(), "p", "h").is_err());
        }
        assert!(
            Model::parse(
                &serde_json::to_string(&vec![entry("p", "h"), entry("p", "h")]).unwrap(),
                "p",
                "h"
            )
            .is_err()
        );
        assert!(Model::parse("not json", "p", "h").is_err());
    }

    #[test]
    fn empty_and_edge_navigation_never_fabricates_a_candidate() {
        let mut empty = Model::parse("[]", "p", "h").unwrap();
        empty.move_cursor(Movement::Last);
        assert!(empty.candidate().is_none());
        let json = serde_json::to_string(&vec![entry("p", "a"), entry("p", "b")]).unwrap();
        let mut model = Model::parse(&json, "missing", "h").unwrap();
        model.move_cursor(Movement::Previous);
        assert_eq!(model.cursor, Some(1));
        model.move_cursor(Movement::Next);
        assert_eq!(model.cursor, Some(1));
        model.move_cursor(Movement::First);
        model.move_cursor(Movement::Previous);
        assert_eq!(model.cursor, Some(0));
        model.move_cursor(Movement::Last);
        assert_eq!(model.visible_start(1), 1);
        assert_eq!(model.visible_start(10), 0);
        assert_eq!(table_text("é\n🐀"), "\\u{e9}\\n\\u{1f400}");
    }
}
