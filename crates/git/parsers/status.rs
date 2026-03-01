use crate::models::status::{FileStatus, FileStatusKind};

pub fn parse_porcelain_v2(buf: &[u8]) -> Result<Vec<FileStatus>, String> {
    let mut result = Vec::new();
    let mut iter = buf.split(|b| *b == 0).peekable();

    while let Some(entry) = iter.next() {
        if entry.is_empty() {
            continue;
        }

        let line = std::str::from_utf8(entry).map_err(|e| e.to_string())?;
        let mut chars = line.chars();

        match chars.next() {
            Some('1') => {
                // 1 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>
                let parts: Vec<&str> = line.splitn(9, ' ').collect();

                if parts.len() < 9 {
                    return Err("invalid type 1 entry".to_string());
                }

                let xy = parts[1];
                let x = xy.as_bytes()[0];
                let y = xy.as_bytes()[1];
                let path = parts[8].to_string();

                let mut status = Vec::new();
                push_xy_status(&mut status, x, y);

                result.push(FileStatus {
                    path,
                    new_path: None,
                    status,
                });
            }

            Some('2') => {
                // 2 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <X><score> <path>\0<origPath>\0
                let parts: Vec<&str> = line.splitn(10, ' ').collect();
                if parts.len() < 10 {
                    return Err("invalid type 2 entry".to_string());
                }

                let xy = parts[1];
                let x = xy.as_bytes()[0];
                let y = xy.as_bytes()[1];
                let new_path = parts[9].to_string();

                // For type 2, the original path comes as the next NUL-separated entry.
                let old_path = iter.next().ok_or("missing old path")?;
                let old_path = std::str::from_utf8(old_path)
                    .map_err(|e| e.to_string())?
                    .to_string();

                let mut status = Vec::new();
                push_xy_status(&mut status, x, y);

                result.push(FileStatus {
                    path: old_path,
                    new_path: Some(new_path),
                    status,
                });
            }

            Some('?') => {
                // ? <path>  (untracked)
                let path = line[2..].to_string();
                result.push(FileStatus {
                    path,
                    new_path: None,
                    status: vec![FileStatusKind::WorktreeNew],
                });
            }

            Some('!') => {
                // ! <path> (ignored) — usually safe to skip, but included if needed
                let path = line[2..].to_string();
                result.push(FileStatus {
                    path,
                    new_path: None,
                    status: Vec::new(),
                });
            }

            _ => {}
        }
    }

    Ok(result)
}

pub fn parse_name_status_z(buf: &[u8]) -> Result<Vec<FileStatus>, String> {
    let mut result = Vec::new();
    let segments: Vec<&[u8]> = buf.split(|b| *b == 0).collect();
    let mut i = 0;

    while i < segments.len() {
        let segment = segments[i];
        if segment.is_empty() {
            i += 1;
            continue;
        }

        let status_str = std::str::from_utf8(segment).map_err(|e| e.to_string())?;
        let first_char = status_str.as_bytes().first().copied().unwrap_or(b'?');

        match first_char {
            b'R' | b'C' => {
                let old_path = segments
                    .get(i + 1)
                    .and_then(|s| std::str::from_utf8(s).ok())
                    .unwrap_or("")
                    .to_string();
                let new_path = segments
                    .get(i + 2)
                    .and_then(|s| std::str::from_utf8(s).ok())
                    .unwrap_or("")
                    .to_string();

                result.push(FileStatus {
                    path: old_path,
                    new_path: Some(new_path),
                    status: vec![FileStatusKind::IndexRenamed],
                });
                i += 3;
            }
            b'A' | b'M' | b'D' | b'T' => {
                let path = segments
                    .get(i + 1)
                    .and_then(|s| std::str::from_utf8(s).ok())
                    .unwrap_or("")
                    .to_string();

                let kind = match first_char {
                    b'A' => FileStatusKind::IndexNew,
                    b'M' => FileStatusKind::IndexModified,
                    b'D' => FileStatusKind::IndexDeleted,
                    b'T' => FileStatusKind::IndexTypechange,
                    _ => unreachable!(),
                };

                result.push(FileStatus {
                    path,
                    new_path: None,
                    status: vec![kind],
                });
                i += 2;
            }
            _ => {
                i += 1;
            }
        }
    }

    Ok(result)
}

pub fn push_xy_status(status: &mut Vec<FileStatusKind>, x: u8, y: u8) {
    match x {
        b'A' => status.push(FileStatusKind::IndexNew),
        b'M' => status.push(FileStatusKind::IndexModified),
        b'D' => status.push(FileStatusKind::IndexDeleted),
        b'R' => status.push(FileStatusKind::IndexRenamed),
        b'T' => status.push(FileStatusKind::IndexTypechange),
        _ => {}
    }

    match y {
        b'A' => status.push(FileStatusKind::WorktreeNew),
        b'M' => status.push(FileStatusKind::WorktreeModified),
        b'D' => status.push(FileStatusKind::WorktreeDeleted),
        b'R' => status.push(FileStatusKind::WorktreeRenamed),
        b'T' => status.push(FileStatusKind::WorktreeTypechange),
        b'X' => status.push(FileStatusKind::WorktreeUnreadable),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Edge case tests ──────────────────────────────────────────────

    #[test]
    fn parse_empty_status() {
        let result = parse_porcelain_v2(&[]).unwrap();
        assert!(result.is_empty());
    }

    // ── push_xy_status tests ─────────────────────────────────────────
    // These are pure function unit tests that don't need real git data

    #[test]
    fn push_xy_status_all_index_statuses() {
        let test_cases = [
            (b'A', FileStatusKind::IndexNew),
            (b'M', FileStatusKind::IndexModified),
            (b'D', FileStatusKind::IndexDeleted),
            (b'R', FileStatusKind::IndexRenamed),
            (b'T', FileStatusKind::IndexTypechange),
        ];

        for (x, expected) in test_cases {
            let mut status = Vec::new();
            push_xy_status(&mut status, x, b'.');
            assert!(
                status
                    .iter()
                    .any(|s| std::mem::discriminant(s) == std::mem::discriminant(&expected)),
                "Expected {:?} for x={}",
                expected,
                x as char
            );
        }
    }

    #[test]
    fn push_xy_status_all_worktree_statuses() {
        let test_cases = [
            (b'A', FileStatusKind::WorktreeNew),
            (b'M', FileStatusKind::WorktreeModified),
            (b'D', FileStatusKind::WorktreeDeleted),
            (b'R', FileStatusKind::WorktreeRenamed),
            (b'T', FileStatusKind::WorktreeTypechange),
            (b'X', FileStatusKind::WorktreeUnreadable),
        ];

        for (y, expected) in test_cases {
            let mut status = Vec::new();
            push_xy_status(&mut status, b'.', y);
            assert!(
                status
                    .iter()
                    .any(|s| std::mem::discriminant(s) == std::mem::discriminant(&expected)),
                "Expected {:?} for y={}",
                expected,
                y as char
            );
        }
    }

    #[test]
    fn push_xy_status_both() {
        let mut status = Vec::new();
        push_xy_status(&mut status, b'M', b'M');
        assert_eq!(status.len(), 2);
    }

    #[test]
    fn push_xy_status_unknown_codes() {
        let mut status = Vec::new();
        push_xy_status(&mut status, b'.', b'.');
        assert!(status.is_empty());

        push_xy_status(&mut status, b'?', b'!');
        assert!(status.is_empty());
    }

    #[test]
    fn parse_name_status_z_regular_and_rename() {
        let bytes = b"M\0src/main.rs\0R100\0old.rs\0new.rs\0";
        let parsed = parse_name_status_z(bytes).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].path, "src/main.rs");
        assert!(parsed[0].new_path.is_none());
        assert_eq!(parsed[1].path, "old.rs");
        assert_eq!(parsed[1].new_path.as_deref(), Some("new.rs"));
    }
}
