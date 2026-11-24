//! Editing the text of a box by a flat character index: paragraphs are
//! joined by `\n`, runs are concatenated, a formula run counts as its
//! source characters. Edits keep the runs' styles.

use crate::model::{Paragraph, Run};

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
