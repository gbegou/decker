//! Reading an OPC package: zip parts and relationships.

use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::Path;

use anyhow::{anyhow, Context, Result};

use super::NS_REL;

pub struct Package {
    parts: HashMap<String, Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct Relationship {
    pub id: String,
    pub kind: String,
    /// Absolute part name, e.g. `ppt/slides/slide1.xml`.
    pub target: String,
}

impl Package {
    pub fn open(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let mut zip =
            zip::ZipArchive::new(Cursor::new(bytes)).with_context(|| format!("{} is not a zip", path.display()))?;
        let mut parts = HashMap::new();
        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            if file.is_dir() {
                continue;
            }
            let mut data = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut data)?;
            parts.insert(file.name().trim_start_matches('/').to_string(), data);
        }
        Ok(Self { parts })
    }

    pub fn part(&self, name: &str) -> Result<&[u8]> {
        self.parts.get(name).map(Vec::as_slice).ok_or_else(|| anyhow!("missing part {name}"))
    }

    pub fn xml(&self, name: &str) -> Result<String> {
        String::from_utf8(self.part(name)?.to_vec()).with_context(|| format!("{name} is not UTF-8"))
    }

    /// The relationships of a part, targets resolved to absolute part names.
    pub fn rels(&self, part: &str) -> Result<Vec<Relationship>> {
        let (dir, file) = match part.rfind('/') {
            Some(i) => (&part[..i], &part[i + 1..]),
            None => ("", part),
        };
        let rels_name = if dir.is_empty() { format!("_rels/{file}.rels") } else { format!("{dir}/_rels/{file}.rels") };
        let Some(bytes) = self.parts.get(&rels_name) else { return Ok(vec![]) };
        let text = std::str::from_utf8(bytes).with_context(|| format!("{rels_name} is not UTF-8"))?;
        let doc = roxmltree::Document::parse(text).with_context(|| format!("parsing {rels_name}"))?;
        let mut out = vec![];
        for node in doc.root_element().children().filter(|n| n.has_tag_name((NS_REL, "Relationship"))) {
            if node.attribute("TargetMode") == Some("External") {
                continue;
            }
            let (Some(id), Some(kind), Some(target)) =
                (node.attribute("Id"), node.attribute("Type"), node.attribute("Target"))
            else {
                continue;
            };
            out.push(Relationship { id: id.into(), kind: kind.into(), target: resolve(dir, target) });
        }
        Ok(out)
    }

    pub fn rel_by_id(&self, part: &str, id: &str) -> Result<String> {
        self.rels(part)?
            .into_iter()
            .find(|r| r.id == id)
            .map(|r| r.target)
            .ok_or_else(|| anyhow!("{part}: no relationship {id}"))
    }

    pub fn rel_by_type(&self, part: &str, kind: &str) -> Result<Option<String>> {
        Ok(self.rels(part)?.into_iter().find(|r| r.kind == kind).map(|r| r.target))
    }
}

/// Resolves `target` (relative to `dir`, or absolute with a leading `/`).
fn resolve(dir: &str, target: &str) -> String {
    if let Some(abs) = target.strip_prefix('/') {
        return abs.to_string();
    }
    let mut segments: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            s => segments.push(s),
        }
    }
    segments.join("/")
}

#[cfg(test)]
mod tests {
    #[test]
    fn resolves_relative_targets() {
        use super::resolve;
        assert_eq!(resolve("ppt/slides", "../media/image1.png"), "ppt/media/image1.png");
        assert_eq!(resolve("ppt", "slides/slide1.xml"), "ppt/slides/slide1.xml");
        assert_eq!(resolve("", "ppt/presentation.xml"), "ppt/presentation.xml");
        assert_eq!(resolve("ppt/slides", "/ppt/media/a.png"), "ppt/media/a.png");
    }
}
