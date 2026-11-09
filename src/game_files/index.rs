//! The game client's file index for one build: where each `res:/` file lives
//! on the resources CDN.
//!
//! A build's app index (`eveonline_<build>.txt` on the binaries host) lists
//! its resfileindexes as `app:/resfileindex[_<name>].txt` rows. Each of those
//! maps `res:/<path>` to a content-addressed location, and a later index wins
//! for a path it shares with an earlier one, as the client's own loader does.

use std::collections::HashMap;

/// The scheme of a game file path.
const RES_ROOT: &str = "res:/";
/// The app index rows that name a resfileindex.
const RES_INDEX_PREFIX: &str = "app:/resfileindex";
/// The client ships only compiled objects; a `.red` path reads its `.black`.
const RED_EXTENSION: &str = ".red";
const BLACK_EXTENSION: &str = ".black";

/// One build's index: lowercase path (without `res:/`) to CDN location.
#[derive(Debug)]
pub struct ResIndex {
    pub build: String,
    locations: HashMap<Box<str>, Box<str>>,
}

impl ResIndex {
    /// Builds the index from resfileindex texts in the app index's order.
    pub fn from_texts<'a>(build: &str, texts: impl IntoIterator<Item = &'a str>) -> Self {
        let mut locations = HashMap::new();
        for text in texts {
            for (path, location) in text.lines().filter_map(parse_row) {
                if let Some(path) = path.strip_prefix(RES_ROOT) {
                    locations.insert(path.to_lowercase().into(), location.into());
                }
            }
        }
        Self {
            build: build.to_owned(),
            locations,
        }
    }

    pub fn len(&self) -> usize {
        self.locations.len()
    }

    /// Where a normalized path lives, reading `.black` for a missing `.red`.
    pub fn locate(&self, path: &str) -> Option<&str> {
        self.locations
            .get(path)
            .or_else(|| {
                let stem = path.strip_suffix(RED_EXTENSION)?;
                self.locations
                    .get(format!("{stem}{BLACK_EXTENSION}").as_str())
            })
            .map(AsRef::as_ref)
    }

    /// The files directly in a normalized folder, by name, sorted.
    pub fn list(&self, folder: &str) -> Vec<&str> {
        let prefix = format!("{folder}/");
        let mut names: Vec<&str> = self
            .locations
            .keys()
            .filter_map(|path| path.strip_prefix(prefix.as_str()))
            .filter(|rest| !rest.contains('/'))
            .collect();
        names.sort_unstable();
        names
    }
}

/// The locations of a build's resfileindexes, in the app index's order.
pub fn res_index_locations(app_index: &str) -> Vec<&str> {
    app_index
        .lines()
        .filter_map(parse_row)
        .filter(|(path, _)| path.starts_with(RES_INDEX_PREFIX) && path.ends_with(".txt"))
        .map(|(_, location)| location)
        .collect()
}

/// A row's logical path and location (the first two comma-separated fields).
fn parse_row(line: &str) -> Option<(&str, &str)> {
    let mut fields = line.trim().split(',');
    let path = fields.next().filter(|path| !path.is_empty())?;
    let location = fields.next().filter(|location| !location.is_empty())?;
    Some((path, location))
}

/// A requested game file path as the index keys it: without `res:/`,
/// lowercase, forward slashes, no empty, `.` or `..` segments. `None` for a
/// path that tries to leave the root.
pub fn normalize(path: &str) -> Option<String> {
    let path = path.trim().replace('\\', "/").to_lowercase();
    let path = path.strip_prefix(RES_ROOT).unwrap_or(&path);
    let mut segments = Vec::new();
    for segment in path.split('/').filter(|segment| !segment.is_empty()) {
        if segment == "." || segment == ".." || segment.contains('\0') {
            return None;
        }
        segments.push(segment);
    }
    (!segments.is_empty()).then(|| segments.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP_INDEX: &str = "\
app:/bin/exefile.exe,aa/aaaa_1,1,10,5,33188
app:/resfileindex.txt,1d/1d34_9c12,9c12,100,50,33188
app:/resfileindex_Windows.txt,7d/7d87_c795,c795,10,5,33188
";

    #[test]
    fn finds_the_resfileindexes_in_order() {
        assert_eq!(
            res_index_locations(APP_INDEX),
            vec!["1d/1d34_9c12", "7d/7d87_c795"]
        );
    }

    #[test]
    fn a_later_index_wins() {
        let index = ResIndex::from_texts(
            "1",
            [
                "res:/dx9/a.dds,11/11_a,a,1,1\nres:/dx9/b.gr2,22/22_b,b,1,1\n",
                "res:/dx9/a.dds,33/33_a,a,1,1\n",
            ],
        );
        assert_eq!(index.locate("dx9/a.dds"), Some("33/33_a"));
        assert_eq!(index.locate("dx9/b.gr2"), Some("22/22_b"));
        assert_eq!(index.len(), 2);
    }

    #[test]
    fn a_red_path_reads_its_black() {
        let index = ResIndex::from_texts("1", ["res:/scene/a.black,44/44_a,a,1,1\n"]);
        assert_eq!(index.locate("scene/a.red"), Some("44/44_a"));
        assert_eq!(index.locate("scene/b.red"), None);
    }

    #[test]
    fn lists_a_folder() {
        let index = ResIndex::from_texts(
            "1",
            [
                "res:/sof/materials/b.black,1/1,x,1,1\nres:/sof/materials/a.black,2/2,x,1,1\nres:/sof/materials/deep/c.black,3/3,x,1,1\nres:/sof/x.black,4/4,x,1,1\n",
            ],
        );
        assert_eq!(index.list("sof/materials"), vec!["a.black", "b.black"]);
        assert!(index.list("nothing").is_empty());
    }

    #[test]
    fn normalizes_paths_like_the_client() {
        assert_eq!(
            normalize("res:/DX9\\Model//Ship/A.GR2").as_deref(),
            Some("dx9/model/ship/a.gr2")
        );
        assert_eq!(
            normalize("dx9/model/a.dds").as_deref(),
            Some("dx9/model/a.dds")
        );
        assert_eq!(normalize("dx9/../etc/passwd"), None);
        assert_eq!(normalize("./a"), None);
        assert_eq!(normalize(""), None);
        assert_eq!(normalize("res:/"), None);
    }
}
