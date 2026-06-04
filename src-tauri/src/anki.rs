//! Anki `.apkg` export for flashcard decks. An `.apkg` is a ZIP archive holding
//! a `collection.anki2` (a SQLite DB in Anki's schema) plus a `media` JSON map
//! (empty here — text-only cards). We build the collection in a temp SQLite file,
//! then pack it into a STORED (uncompressed) ZIP by hand, so no new crates are
//! pulled in. The legacy schema (`ver = 11`) imports cleanly into current Anki.

use crate::db::now_ms;
use crate::error::{Error, Result};
use rusqlite::{params, Connection};
use std::path::Path;

// ---- SHA-1 (for Anki's note checksum `csum`) --------------------------------

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let ml = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&ml.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, wi) in w.iter_mut().take(16).enumerate() {
            *wi = u32::from_be_bytes([chunk[i * 4], chunk[i * 4 + 1], chunk[i * 4 + 2], chunk[i * 4 + 3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let tmp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = tmp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for (i, hi) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&hi.to_be_bytes());
    }
    out
}

/// Anki field checksum: the integer of the first 8 hex digits (first 4 bytes) of
/// the SHA-1 of the (HTML-stripped) first field. Used for duplicate detection.
fn field_checksum(field: &str) -> i64 {
    let stripped = strip_html(field);
    let d = sha1(stripped.as_bytes());
    u32::from_be_bytes([d[0], d[1], d[2], d[3]]) as i64
}

/// Minimal HTML tag strip — fronts are usually plain text, but Anki computes the
/// checksum on stripped content, so mirror that for stable dedupe.
fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

// ---- CRC32 (for the ZIP entries) --------------------------------------------

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

// ---- minimal STORED ZIP writer ----------------------------------------------

/// Pack named entries into a ZIP using the STORED (no-compression) method.
fn zip_store(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    let mut offsets: Vec<u32> = Vec::new();

    for (name, data) in entries {
        offsets.push(out.len() as u32);
        let crc = crc32(data);
        let nlen = name.len() as u16;
        let sz = data.len() as u32;
        // local file header
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // method = stored
        out.extend_from_slice(&0u16.to_le_bytes()); // mod time
        out.extend_from_slice(&0u16.to_le_bytes()); // mod date
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&sz.to_le_bytes()); // compressed size
        out.extend_from_slice(&sz.to_le_bytes()); // uncompressed size
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra len
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
    }

    for (idx, (name, data)) in entries.iter().enumerate() {
        let crc = crc32(data);
        let nlen = name.len() as u16;
        let sz = data.len() as u32;
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // version made by
        central.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central.extend_from_slice(&0u16.to_le_bytes()); // flags
        central.extend_from_slice(&0u16.to_le_bytes()); // method
        central.extend_from_slice(&0u16.to_le_bytes()); // mod time
        central.extend_from_slice(&0u16.to_le_bytes()); // mod date
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&sz.to_le_bytes());
        central.extend_from_slice(&sz.to_le_bytes());
        central.extend_from_slice(&nlen.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // extra len
        central.extend_from_slice(&0u16.to_le_bytes()); // comment len
        central.extend_from_slice(&0u16.to_le_bytes()); // disk start
        central.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        central.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        central.extend_from_slice(&offsets[idx].to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }

    let cd_offset = out.len() as u32;
    let cd_size = central.len() as u32;
    out.extend_from_slice(&central);
    // end of central directory
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // disk num
    out.extend_from_slice(&0u16.to_le_bytes()); // disk w/ cd
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment len
    out
}

// ---- collection.anki2 builder -----------------------------------------------

const COL_SCHEMA: &str = "
CREATE TABLE col (id integer PRIMARY KEY, crt integer NOT NULL, mod integer NOT NULL,
  scm integer NOT NULL, ver integer NOT NULL, dty integer NOT NULL, usn integer NOT NULL,
  ls integer NOT NULL, conf text NOT NULL, models text NOT NULL, decks text NOT NULL,
  dconf text NOT NULL, tags text NOT NULL);
CREATE TABLE notes (id integer PRIMARY KEY, guid text NOT NULL, mid integer NOT NULL,
  mod integer NOT NULL, usn integer NOT NULL, tags text NOT NULL, flds text NOT NULL,
  sfld text NOT NULL, csum integer NOT NULL, flags integer NOT NULL, data text NOT NULL);
CREATE TABLE cards (id integer PRIMARY KEY, nid integer NOT NULL, did integer NOT NULL,
  ord integer NOT NULL, mod integer NOT NULL, usn integer NOT NULL, type integer NOT NULL,
  queue integer NOT NULL, due integer NOT NULL, ivl integer NOT NULL, factor integer NOT NULL,
  reps integer NOT NULL, lapses integer NOT NULL, left integer NOT NULL, odue integer NOT NULL,
  odid integer NOT NULL, flags integer NOT NULL, data text NOT NULL);
CREATE TABLE revlog (id integer PRIMARY KEY, cid integer NOT NULL, usn integer NOT NULL,
  ease integer NOT NULL, ivl integer NOT NULL, lastIvl integer NOT NULL, factor integer NOT NULL,
  time integer NOT NULL, type integer NOT NULL);
CREATE TABLE graves (usn integer NOT NULL, oid integer NOT NULL, type integer NOT NULL);
CREATE INDEX ix_notes_csum on notes (csum);
CREATE INDEX ix_cards_nid on cards (nid);
CREATE INDEX ix_cards_sched on cards (did, queue, due);
CREATE INDEX ix_revlog_cid on revlog (cid);
";

/// Build a `collection.anki2` SQLite file at `path` containing one Basic note +
/// card per (front, back) pair, all in a deck named `deck_name`.
fn build_collection(path: &Path, deck_name: &str, cards: &[(String, String)]) -> Result<()> {
    let conn = Connection::open(path)?;
    conn.execute_batch(COL_SCHEMA)?;

    let now = now_ms();
    let crt = now / 1000;
    let mid = now; // model id
    let did = now + 1; // deck id

    let model = serde_json::json!({
        mid.to_string(): {
            "id": mid, "name": "Cortex Basic", "type": 0, "mod": crt, "usn": -1,
            "sortf": 0, "did": did, "latexPre": "", "latexPost": "", "latexsvg": false,
            "css": ".card{font-family:arial;font-size:20px;text-align:center;color:black;background:white;}",
            "flds": [
                {"name":"Front","ord":0,"sticky":false,"rtl":false,"font":"Arial","size":20,"media":[]},
                {"name":"Back","ord":1,"sticky":false,"rtl":false,"font":"Arial","size":20,"media":[]}
            ],
            "tmpls": [
                {"name":"Card 1","ord":0,"qfmt":"{{Front}}",
                 "afmt":"{{FrontSide}}\n\n<hr id=answer>\n\n{{Back}}","did":null,"bqfmt":"","bafmt":""}
            ],
            "req": [[0, "any", [0]]], "tags": [], "vers": []
        }
    });
    let decks = serde_json::json!({
        "1": {"id":1,"name":"Default","mod":crt,"usn":-1,"lrnToday":[0,0],"revToday":[0,0],
              "newToday":[0,0],"timeToday":[0,0],"collapsed":false,"browserCollapsed":false,
              "desc":"","dyn":0,"conf":1,"extendNew":0,"extendRev":0},
        did.to_string(): {"id":did,"name":deck_name,"mod":crt,"usn":-1,"lrnToday":[0,0],
              "revToday":[0,0],"newToday":[0,0],"timeToday":[0,0],"collapsed":false,
              "browserCollapsed":false,"desc":"","dyn":0,"conf":1,"extendNew":0,"extendRev":0}
    });
    let dconf = serde_json::json!({
        "1": {"id":1,"name":"Default","mod":0,"usn":0,"maxTaken":60,"autoplay":true,
              "timer":0,"replayq":true,"new":{"bury":false,"delays":[1.0,10.0],"initialFactor":2500,
              "ints":[1,4,0],"order":1,"perDay":20},"rev":{"bury":false,"ease4":1.3,"ivlFct":1.0,
              "maxIvl":36500,"perDay":200,"hardFactor":1.2},"lapse":{"delays":[10.0],"leechAction":1,
              "leechFails":8,"minInt":1,"mult":0.0},"dyn":false}
    });
    let conf = serde_json::json!({
        "nextPos":1,"estTimes":true,"activeDecks":[1],"sortType":"noteFld","timeLim":0,
        "sortBackwards":false,"addToCur":true,"curDeck":did,"newBury":true,"newSpread":0,
        "dueCounts":true,"curModel":mid.to_string(),"collapseTime":1200
    });

    conn.execute(
        "INSERT INTO col (id,crt,mod,scm,ver,dty,usn,ls,conf,models,decks,dconf,tags)
         VALUES (1,?1,?2,?2,11,0,0,0,?3,?4,?5,?6,'{}')",
        params![crt, now, conf.to_string(), model.to_string(), decks.to_string(), dconf.to_string()],
    )?;

    for (idx, (front, back)) in cards.iter().enumerate() {
        let nid = now + 100 + idx as i64;
        let cid = now + 100_000 + idx as i64;
        let flds = format!("{front}\u{001f}{back}");
        let guid = format!("crtx{:x}", nid); // unique within the file
        conn.execute(
            "INSERT INTO notes (id,guid,mid,mod,usn,tags,flds,sfld,csum,flags,data)
             VALUES (?1,?2,?3,?4,-1,'',?5,?6,?7,0,'')",
            params![nid, guid, mid, crt, flds, front, field_checksum(front)],
        )?;
        // New card: type/queue 0, due = position (1-based), default ease 2500.
        conn.execute(
            "INSERT INTO cards (id,nid,did,ord,mod,usn,type,queue,due,ivl,factor,reps,lapses,left,odue,odid,flags,data)
             VALUES (?1,?2,?3,0,?4,-1,0,0,?5,0,2500,0,0,0,0,0,0,'')",
            params![cid, nid, did, crt, (idx as i64) + 1],
        )?;
    }
    conn.pragma_update(None, "user_version", 0)?;
    drop(conn);
    Ok(())
}

/// Build a complete `.apkg` for the given cards and write it to `dest`.
pub fn export_apkg(dest: &Path, deck_name: &str, cards: &[(String, String)]) -> Result<()> {
    if cards.is_empty() {
        return Err(Error::Other("no flashcards to export".into()));
    }
    // Build collection.anki2 in a temp file, then read its bytes.
    let tmp = std::env::temp_dir().join(format!("cortex-anki-{}.anki2", now_ms()));
    build_collection(&tmp, deck_name, cards)?;
    let col_bytes = std::fs::read(&tmp).map_err(Error::Io)?;
    let _ = std::fs::remove_file(&tmp);

    let zip = zip_store(&[
        ("collection.anki2", col_bytes),
        ("media", b"{}".to_vec()),
    ]);
    std::fs::write(dest, zip).map_err(Error::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_known_vector() {
        let d = sha1(b"abc");
        let hex: String = d.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn crc32_known_vector() {
        // CRC-32 of "123456789" is 0xCBF43926.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn apkg_is_a_valid_zip_with_anki_collection() {
        let dir = std::env::temp_dir().join(format!("cortex-apkg-test-{}", now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("deck.apkg");
        let cards = vec![
            ("What is ATP?".to_string(), "Adenosine triphosphate".to_string()),
            ("Powerhouse of the cell?".to_string(), "Mitochondria".to_string()),
        ];
        export_apkg(&dest, "Biology", &cards).unwrap();

        let bytes = std::fs::read(&dest).unwrap();
        // ZIP local-file signature at the start, EOCD signature near the end.
        assert_eq!(&bytes[0..4], &[0x50, 0x4b, 0x03, 0x04]);
        assert!(bytes.windows(4).any(|w| w == [0x50, 0x4b, 0x05, 0x06]));
        // Contains both archive members.
        assert!(bytes.windows(b"collection.anki2".len()).any(|w| w == b"collection.anki2"));
        assert!(bytes.windows(b"media".len()).any(|w| w == b"media"));

        // The embedded collection.anki2 must be a real SQLite DB with 2 notes/cards.
        let tmp = dir.join("roundtrip.anki2");
        build_collection(&tmp, "Biology", &cards).unwrap();
        let conn = Connection::open(&tmp).unwrap();
        let notes: i64 = conn.query_row("SELECT count(*) FROM notes", [], |r| r.get(0)).unwrap();
        let cnt: i64 = conn.query_row("SELECT count(*) FROM cards", [], |r| r.get(0)).unwrap();
        assert_eq!(notes, 2);
        assert_eq!(cnt, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
