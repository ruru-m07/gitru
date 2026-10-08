"""Apply one hash-verified repair to the pinned regression's HTTP test fixture."""
import hashlib
from pathlib import Path

import prepare

OLD = '''            s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            s.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut b = [0u8; 8192];
            let n = s.read(&mut b).unwrap();
            let _ = s.write_all(response.as_bytes());
            requests.push(String::from_utf8(b[..n].to_vec()).unwrap());'''
NEW = '''            let deadline = start + Duration::from_secs(8);
            let request = qualification_http_request(&mut s, deadline).unwrap();
            s.set_write_timeout(Some(
                deadline.checked_duration_since(std::time::Instant::now())
                    .filter(|remaining| !remaining.is_zero())
                    .expect("Finite fixture response deadline")
                    .min(Duration::from_secs(2)),
            )).unwrap();
            s.write_all(response.as_bytes()).unwrap();
            requests.push(String::from_utf8(request).unwrap());'''


def apply(destination: Path):
    pins = prepare.PINS["http_fixture_adaptation"]
    path = destination / pins["path"]
    prepare.verify(path, pins["source_sha256"])
    helper = prepare.ROOT / "inbox_http_fixture.rs"
    prepare.verify(helper, pins["helper_sha256"])
    contents = path.read_text(encoding="utf-8")
    if contents.count(OLD) != 1:
        raise RuntimeError("Expected one reviewed HTTP fixture body")
    changed = contents.replace(OLD, NEW) + "\n" + helper.read_text(encoding="utf-8")
    # Check the entire reviewed result before changing the copied fixture.
    if hashlib.sha256(changed.encode("utf-8")).hexdigest() != pins["patched_sha256"]:
        raise RuntimeError("HTTP fixture adaptation differs from reviewed result")
    path.write_bytes(changed.encode("utf-8"))
