//! Editing the text of a box by a flat character index: paragraphs are
//! joined by `\n`, runs are concatenated, a formula run counts as its
//! source characters. Edits keep the runs' styles.

use crate::model::{Align, Color, Paragraph, Run};
use serde::{Deserialize, Serialize};

/// The flat text of paragraphs, `\n` between paragraphs.
pub fn flat_text(paragraphs: &[Paragraph]) -> String {
    paragraphs.iter().map(Paragraph::text).collect::<Vec<_>>().join("\n")
}

/// The number of characters, paragraph breaks included.
pub fn len(paragraphs: &[Paragraph]) -> usize {
    flat_text(paragraphs).chars().count()
}

/// Where a flat index falls: paragraph, run and character offset in it.
/// An index on a paragraph break maps to the end of that paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    pub paragraph: usize,
    pub run: usize,
    pub offset: usize,
}

/// Locates `at` (clamped to the end). At a boundary between two runs the
/// previous run wins, so that typing there continues its style. An empty
/// paragraph gives run 0, offset 0, whether or not the run exists.
pub fn locate(paragraphs: &[Paragraph], at: usize) -> Pos {
    let mut remaining = at;
    for (pi, p) in paragraphs.iter().enumerate() {
        let plen: usize = p.runs.iter().map(|r| r.text.chars().count()).sum();
        if remaining <= plen {
            let mut off = remaining;
            for (ri, r) in p.runs.iter().enumerate() {
                let rlen = r.text.chars().count();
                if off <= rlen {
                    return Pos { paragraph: pi, run: ri, offset: off };
                }
                off -= rlen;
            }
            return Pos { paragraph: pi, run: 0, offset: 0 };
        }
        remaining -= plen + 1;
    }
    let last = paragraphs.len().saturating_sub(1);
    let run = paragraphs.get(last).map(|p| p.runs.len().saturating_sub(1)).unwrap_or(0);
    let offset = paragraphs.get(last).and_then(|p| p.runs.last()).map(|r| r.text.chars().count()).unwrap_or(0);
    Pos { paragraph: last, run, offset }
}

fn byte_at(s: &str, chars: usize) -> usize {
    s.char_indices().nth(chars).map(|(b, _)| b).unwrap_or(s.len())
}

/// Inserts `text` at `at`; a `\n` starts a new paragraph with the same
/// alignment and bullet. Returns the index after the inserted text.
pub fn insert(paragraphs: &mut Vec<Paragraph>, at: usize, text: &str) -> usize {
    if paragraphs.is_empty() {
        paragraphs.push(Paragraph::default());
    }
    let mut at = at.min(len(paragraphs));
    for c in text.chars() {
        let pos = locate(paragraphs, at);
        let p = &mut paragraphs[pos.paragraph];
        if p.runs.is_empty() {
            // Inherit the style of the previous paragraph's last run.
            let style = paragraphs[..pos.paragraph].iter().rev().find_map(|q| q.runs.last().cloned());
            paragraphs[pos.paragraph].runs.push(Run { text: String::new(), ..style.unwrap_or_default() });
        }
        if c == '\n' {
            let p = &mut paragraphs[pos.paragraph];
            let run = &mut p.runs[pos.run];
            let b = byte_at(&run.text, pos.offset);
            let tail_text = run.text.split_off(b);
            let mut tail = p.runs.split_off(pos.run + 1);
            let mut first = p.runs[pos.run].clone();
            first.text = tail_text;
            tail.insert(0, first);
            let new = Paragraph { runs: tail, align: p.align, bullet: p.bullet };
            paragraphs.insert(pos.paragraph + 1, new);
        } else {
            let run = &mut paragraphs[pos.paragraph].runs[pos.run];
            let b = byte_at(&run.text, pos.offset);
            run.text.insert(b, c);
        }
        at += 1;
    }
    at
}

/// Deletes the characters in `from..to`; a deleted paragraph break merges
/// the two paragraphs. Returns `from`.
pub fn delete(paragraphs: &mut Vec<Paragraph>, from: usize, to: usize) -> usize {
    let total = len(paragraphs);
    let (from, to) = (from.min(total), to.min(total));
    if from >= to {
        return from;
    }
    for _ in from..to {
        let pos = locate(paragraphs, from);
        let p = &paragraphs[pos.paragraph];
        let at_end = p.runs.get(pos.run).map(|r| pos.offset >= r.text.chars().count()).unwrap_or(true)
            && pos.run + 1 >= p.runs.len();
        if at_end {
            // The character after the caret is the paragraph break.
            if pos.paragraph + 1 < paragraphs.len() {
                let next = paragraphs.remove(pos.paragraph + 1);
                paragraphs[pos.paragraph].runs.extend(next.runs);
            }
        } else {
            // Inside a run, or at the boundary to the next run.
            let p = &mut paragraphs[pos.paragraph];
            let (ri, off) = if pos.offset >= p.runs[pos.run].text.chars().count() {
                (pos.run + 1, 0)
            } else {
                (pos.run, pos.offset)
            };
            let run = &mut p.runs[ri];
            let b = byte_at(&run.text, off);
            if b < run.text.len() {
                run.text.remove(b);
            }
            if run.text.is_empty() && p.runs.len() > 1 {
                p.runs.remove(ri);
            }
        }
    }
    from
}

/// A change of run style over a range. Unset fields keep the run's value;
/// `size` 0, `color` "" and `font` "" reset to the default.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StyleChange {
    #[serde(default)]
    pub bold: Option<bool>,
    #[serde(default)]
    pub italic: Option<bool>,
    /// Toggle bold: on unless every run of the range is bold already.
    #[serde(default)]
    pub toggle_bold: bool,
    #[serde(default)]
    pub toggle_italic: bool,
    #[serde(default)]
    pub size: Option<f64>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub font: Option<String>,
}

/// A change of paragraph style over a range. `bullet` -1 removes the
/// bullet, 0 and above sets the level.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ParagraphChange {
    #[serde(default)]
    pub align: Option<Align>,
    #[serde(default)]
    pub bullet: Option<i32>,
}

/// The style found over a range, for a toolbar: `bold` and `italic` are
/// true when every run has them; the others come from the first run.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct StyleInfo {
    pub bold: bool,
    pub italic: bool,
    pub size: Option<f64>,
    pub color: Option<Color>,
    pub font: Option<String>,
    pub align: Align,
    pub bullet: Option<u8>,
}

/// Makes a run boundary at flat index `at`, splitting the run it falls in.
fn split_at(paragraphs: &mut [Paragraph], at: usize) {
    let pos = locate(paragraphs, at);
    let Some(p) = paragraphs.get_mut(pos.paragraph) else { return };
    let Some(run) = p.runs.get_mut(pos.run) else { return };
    let len = run.text.chars().count();
    if pos.offset == 0 || pos.offset >= len {
        return;
    }
    let b = byte_at(&run.text, pos.offset);
    let tail = run.text.split_off(b);
    let mut second = run.clone();
    second.text = tail;
    p.runs.insert(pos.run + 1, second);
}

/// The (paragraph, run) pairs whose text lies within `from..to`, after
/// splitting at both ends. An empty range selects the run at `from`.
fn runs_in(paragraphs: &mut [Paragraph], from: usize, to: usize) -> Vec<(usize, usize)> {
    let (from, to) = (from.min(to), from.max(to));
    split_at(paragraphs, from);
    split_at(paragraphs, to);
    let mut out = vec![];
    let mut index = 0usize;
    for (pi, p) in paragraphs.iter().enumerate() {
        for (ri, r) in p.runs.iter().enumerate() {
            let len = r.text.chars().count();
            let (start, end) = (index, index + len);
            let inside = if from == to {
                start <= from && from <= end && (len > 0 || p.runs.len() == 1)
            } else {
                start < to && end > from
            };
            if inside && (from != to || out.is_empty()) {
                out.push((pi, ri));
            }
            index = end;
        }
        index += 1;
    }
    out
}

/// Merges adjacent runs of equal style and drops empty runs (a paragraph
/// keeps one run when it would have none left).
pub fn normalize(paragraphs: &mut [Paragraph]) {
    for p in paragraphs.iter_mut() {
        let mut merged: Vec<Run> = vec![];
        for r in p.runs.drain(..) {
            match merged.last_mut() {
                Some(last) if same_style(last, &r) => last.text.push_str(&r.text),
                _ => merged.push(r),
            }
        }
        let keep_one = merged.first().cloned();
        merged.retain(|r| !r.text.is_empty());
        if merged.is_empty() {
            if let Some(mut r) = keep_one {
                r.text.clear();
                merged.push(r);
            }
        }
        p.runs = merged;
    }
}

fn same_style(a: &Run, b: &Run) -> bool {
    a.bold == b.bold
        && a.italic == b.italic
        && a.size == b.size
        && a.color == b.color
        && a.font == b.font
        && a.math == b.math
}

/// Applies `change` to the runs in `from..to` (the whole paragraph set
/// when `from == to == usize::MAX`), creating run boundaries as needed.
pub fn apply_style(paragraphs: &mut [Paragraph], from: usize, to: usize, change: &StyleChange) {
    let targets = if from == usize::MAX {
        paragraphs.iter().enumerate().flat_map(|(pi, p)| (0..p.runs.len()).map(move |ri| (pi, ri))).collect()
    } else {
        runs_in(paragraphs, from, to)
    };
    let all_bold = targets.iter().all(|&(pi, ri)| paragraphs[pi].runs[ri].bold);
    let all_italic = targets.iter().all(|&(pi, ri)| paragraphs[pi].runs[ri].italic);
    for (pi, ri) in targets {
        let r = &mut paragraphs[pi].runs[ri];
        if let Some(b) = change.bold {
            r.bold = b;
        }
        if change.toggle_bold {
            r.bold = !all_bold;
        }
        if let Some(i) = change.italic {
            r.italic = i;
        }
        if change.toggle_italic {
            r.italic = !all_italic;
        }
        if let Some(size) = change.size {
            r.size = if size > 0.0 { Some(size) } else { None };
        }
        if let Some(c) = &change.color {
            r.color = if c.is_empty() { None } else { Color::from_hex(c).ok() };
        }
        if let Some(f) = &change.font {
            r.font = if f.is_empty() { None } else { Some(f.clone()) };
        }
    }
    normalize(paragraphs);
}

/// Applies `change` to the paragraphs touched by `from..to` (all of them
/// when `from == usize::MAX`).
pub fn apply_paragraph(paragraphs: &mut [Paragraph], from: usize, to: usize, change: &ParagraphChange) {
    let (from, to) = (from.min(to), from.max(to));
    let mut index = 0usize;
    for p in paragraphs.iter_mut() {
        let len: usize = p.runs.iter().map(|r| r.text.chars().count()).sum();
        let (start, end) = (index, index + len);
        let touched = from == usize::MAX || (start <= to && end >= from);
        if touched {
            if let Some(a) = change.align {
                p.align = a;
            }
            if let Some(b) = change.bullet {
                p.bullet = if b < 0 { None } else { Some(b.min(8) as u8) };
            }
        }
        index = end + 1;
    }
}

/// The style over `from..to` (the first run's for the single values).
pub fn style_info(paragraphs: &[Paragraph], from: usize, to: usize) -> StyleInfo {
    let mut copy: Vec<Paragraph> = paragraphs.to_vec();
    let targets = if from == usize::MAX {
        copy.iter().enumerate().flat_map(|(pi, p)| (0..p.runs.len()).map(move |ri| (pi, ri))).collect::<Vec<_>>()
    } else {
        runs_in(&mut copy, from, to)
    };
    let first = targets.first().map(|&(pi, ri)| copy[pi].runs[ri].clone());
    let pos = locate(&copy, if from == usize::MAX { 0 } else { from.min(to) });
    let para = copy.get(pos.paragraph);
    StyleInfo {
        bold: !targets.is_empty() && targets.iter().all(|&(pi, ri)| copy[pi].runs[ri].bold),
        italic: !targets.is_empty() && targets.iter().all(|&(pi, ri)| copy[pi].runs[ri].italic),
        size: first.as_ref().and_then(|r| r.size),
        color: first.as_ref().and_then(|r| r.color),
        font: first.as_ref().and_then(|r| r.font.clone()),
        align: para.map(|p| p.align).unwrap_or_default(),
        bullet: para.and_then(|p| p.bullet),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bold(t: &str) -> Run {
        Run { text: t.into(), bold: true, ..Default::default() }
    }

    fn sample() -> Vec<Paragraph> {
        vec![
            Paragraph { runs: vec![Run::plain("Hello "), bold("world")], ..Default::default() },
            Paragraph { runs: vec![Run::plain("second")], bullet: Some(0), ..Default::default() },
        ]
    }

    #[test]
    fn flat_and_locate() {
        let p = sample();
        assert_eq!(flat_text(&p), "Hello world\nsecond");
        assert_eq!(len(&p), 18);
        assert_eq!(locate(&p, 0), Pos { paragraph: 0, run: 0, offset: 0 });
        assert_eq!(locate(&p, 6), Pos { paragraph: 0, run: 0, offset: 6 }, "end of first run, start of bold");
        assert_eq!(locate(&p, 7), Pos { paragraph: 0, run: 1, offset: 1 });
        assert_eq!(locate(&p, 11), Pos { paragraph: 0, run: 1, offset: 5 }, "on the paragraph break");
        assert_eq!(locate(&p, 12), Pos { paragraph: 1, run: 0, offset: 0 });
        assert_eq!(locate(&p, 99), Pos { paragraph: 1, run: 0, offset: 6 });
    }

    #[test]
    fn insert_keeps_styles_and_splits_paragraphs() {
        let mut p = sample();
        assert_eq!(insert(&mut p, 8, "XY"), 10);
        assert_eq!(p[0].runs[1].text, "woXYrld", "typed inside the bold run stays bold");
        insert(&mut p, 5, "\n");
        assert_eq!(p.len(), 3);
        assert_eq!(p[0].runs[0].text, "Hello");
        assert_eq!(p[1].runs[0].text, " ");
        assert!(p[1].runs[1].bold);
        assert_eq!(flat_text(&p), "Hello\n world\nsecond".replace("world", "woXYrld"));
        let mut empty = vec![];
        insert(&mut empty, 0, "a\nb");
        assert_eq!(flat_text(&empty), "a\nb");
    }

    #[test]
    fn styles_split_and_merge_runs() {
        let mut p = sample(); // "Hello world\nsecond", "world" bold
        apply_style(&mut p, 2, 4, &StyleChange { italic: Some(true), ..Default::default() });
        let texts: Vec<(String, bool, bool)> = p[0].runs.iter().map(|r| (r.text.clone(), r.bold, r.italic)).collect();
        assert_eq!(
            texts,
            [
                ("He".into(), false, false),
                ("ll".into(), false, true),
                ("o ".into(), false, false),
                ("world".into(), true, false)
            ]
        );
        // Removing italic again merges the three plain runs back into one.
        apply_style(&mut p, 2, 4, &StyleChange { italic: Some(false), ..Default::default() });
        assert_eq!(p[0].runs.len(), 2);
        // Toggle bold over a mixed range makes everything bold, then over an all-bold range removes it.
        apply_style(&mut p, 0, 11, &StyleChange { toggle_bold: true, ..Default::default() });
        assert!(p[0].runs.iter().all(|r| r.bold) && p[0].runs.len() == 1);
        apply_style(&mut p, 0, 11, &StyleChange { toggle_bold: true, ..Default::default() });
        assert!(p[0].runs.iter().all(|r| !r.bold));
        // Whole element: size and color on every run, reset with 0 and "".
        apply_style(
            &mut p,
            usize::MAX,
            usize::MAX,
            &StyleChange { size: Some(24.0), color: Some("#FF0000".into()), ..Default::default() },
        );
        assert!(p.iter().flat_map(|q| &q.runs).all(|r| r.size == Some(24.0) && r.color == Some(Color::rgb(255, 0, 0))));
        apply_style(
            &mut p,
            usize::MAX,
            usize::MAX,
            &StyleChange { size: Some(0.0), color: Some(String::new()), ..Default::default() },
        );
        assert!(p.iter().flat_map(|q| &q.runs).all(|r| r.size.is_none() && r.color.is_none()));
        // Paragraph changes touch the paragraphs of the range only.
        apply_paragraph(&mut p, 13, 13, &ParagraphChange { align: Some(Align::Center), bullet: Some(2) });
        assert_eq!((p[0].align, p[1].align, p[1].bullet), (Align::Left, Align::Center, Some(2)));
        apply_paragraph(&mut p, usize::MAX, usize::MAX, &ParagraphChange { bullet: Some(-1), ..Default::default() });
        assert!(p.iter().all(|q| q.bullet.is_none()));
        let info = style_info(&sample(), 7, 9);
        assert!(info.bold && !info.italic);
        assert_eq!(style_info(&sample(), 0, 11).bold, false, "mixed range is not all bold");
        assert_eq!(style_info(&sample(), 12, 12).bullet, Some(0));
    }

    #[test]
    fn delete_merges_paragraphs_and_drops_empty_runs() {
        let mut p = sample();
        delete(&mut p, 11, 12);
        assert_eq!(p.len(), 1);
        assert_eq!(flat_text(&p), "Hello worldsecond");
        let mut p = sample();
        delete(&mut p, 6, 11);
        assert_eq!(p[0].runs.len(), 1, "the emptied bold run is removed");
        assert_eq!(flat_text(&p), "Hello \nsecond");
        let mut p = sample();
        delete(&mut p, 0, 99);
        assert_eq!(flat_text(&p), "");
        assert_eq!(p.len(), 1);
    }
}
