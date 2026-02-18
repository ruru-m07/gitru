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
                let mut parts = line.split_whitespace();
                parts.next(); // "2"

                let xy = parts.next().ok_or("missing XY")?;
                let x = xy.as_bytes()[0];
                let y = xy.as_bytes()[1];

                // Paths come from iterator for type 2
                let new_path = iter.next().ok_or("missing new path")?;
                let old_path = iter.next().ok_or("missing old path")?;

                let new_path = std::str::from_utf8(new_path)
                    .map_err(|e| e.to_string())?
                    .to_string();
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
