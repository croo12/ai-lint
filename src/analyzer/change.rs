use std::ops::Range;

/// An absent file is different from an unavailable baseline or an empty file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviousSource {
    Unknown,
    Absent,
    Present(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Unknown,
    Added,
    Modified,
    Unchanged,
}

/// The smallest single pair of UTF-8 byte ranges enclosing all edits.
/// Ends are exclusive; an insertion or deletion has an empty range on one side.
/// Separate edits may include unchanged text between them; these are not diff hunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedRange {
    pub before: Range<usize>,
    pub after: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeMetadata {
    pub kind: ChangeKind,
    pub previous_bytes: Option<usize>,
    pub current_bytes: usize,
    /// None for unknown or unchanged contents.
    pub range: Option<ChangedRange>,
}

impl ChangeMetadata {
    pub(super) fn between(previous: &PreviousSource, current: &str) -> Self {
        let (kind, previous_bytes, range) = match previous {
            PreviousSource::Unknown => (ChangeKind::Unknown, None, None),
            PreviousSource::Absent => (
                ChangeKind::Added,
                None,
                Some(ChangedRange {
                    before: 0..0,
                    after: 0..current.len(),
                }),
            ),
            PreviousSource::Present(previous) if previous == current => {
                (ChangeKind::Unchanged, Some(previous.len()), None)
            }
            PreviousSource::Present(previous) => (
                ChangeKind::Modified,
                Some(previous.len()),
                Some(changed_range(previous, current)),
            ),
        };
        Self {
            kind,
            previous_bytes,
            current_bytes: current.len(),
            range,
        }
    }
}

fn changed_range(previous: &str, current: &str) -> ChangedRange {
    // Compare characters so a shared UTF-8 byte prefix never splits a character.
    let prefix: usize = previous
        .chars()
        .zip(current.chars())
        .take_while(|(before, after)| before == after)
        .map(|(character, _)| character.len_utf8())
        .sum();
    let suffix: usize = previous[prefix..]
        .chars()
        .rev()
        .zip(current[prefix..].chars().rev())
        .take_while(|(before, after)| before == after)
        .map(|(character, _)| character.len_utf8())
        .sum();

    ChangedRange {
        before: prefix..previous.len() - suffix,
        after: prefix..current.len() - suffix,
    }
}
