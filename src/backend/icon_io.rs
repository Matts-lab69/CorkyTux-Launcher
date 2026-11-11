//! Read local image data once, through a nonblocking regular-file descriptor.

use gdk_pixbuf::prelude::*;
use gdk_pixbuf::Pixbuf;
use std::fs::OpenOptions;
use std::io::{Cursor, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// Follow ordinary symlinks, but validate and read the same opened descriptor.
/// O_NONBLOCK prevents waiting for a FIFO writer; regular-file reads can still
/// incur ordinary filesystem latency. No image size or dimension cap is added.
pub(crate) fn read_icon_bytes(path: &Path) -> Option<glib::Bytes> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY)
        .open(path)
        .ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut data = Vec::new();
    file.read_to_end(&mut data).ok()?;
    Some(glib::Bytes::from_owned(data))
}

/// Preserve content sniffing and the PNG-compressed ICO fallback without
/// reopening a pathname after its descriptor was checked.
pub(crate) fn decode_icon_bytes(data: &glib::Bytes) -> Option<Pixbuf> {
    Pixbuf::from_read(Cursor::new(data.clone()))
        .ok()
        .or_else(|| decode_ico_pixbuf(data.as_ref()))
}

fn decode_ico_pixbuf(data: &[u8]) -> Option<Pixbuf> {
    if data.len() < 6 || data[0..4] != [0, 0, 1, 0] {
        return None;
    }
    let count = usize::from(u16::from_le_bytes([data[4], data[5]]));
    let mut best: Option<(u32, usize, usize)> = None;
    // Retain the existing ICO entry-search policy; this is not an image-size cap.
    for i in 0..count.min(64) {
        let off = 6 + i * 16;
        if data.len() < off + 16 {
            break;
        }
        let width = u32::from(data[off]);
        let height = u32::from(data[off + 1]);
        let size =
            usize::try_from(u32::from_le_bytes(data[off + 8..off + 12].try_into().ok()?)).ok()?;
        let start = usize::try_from(u32::from_le_bytes(
            data[off + 12..off + 16].try_into().ok()?,
        ))
        .ok()?;
        let Some(end) = start.checked_add(size) else {
            continue;
        };
        let Some(entry) = data.get(start..end) else {
            continue;
        };
        if size < 8 || !entry.starts_with(&[0x89, b'P', b'N', b'G']) {
            continue;
        }
        let width = if width == 0 { 256 } else { width };
        let height = if height == 0 { 256 } else { height };
        let area = width * height;
        if best.map(|(old, _, _)| area > old).unwrap_or(true) {
            best = Some((area, start, end));
        }
    }
    let (_, start, end) = best?;
    let loader = gdk_pixbuf::PixbufLoader::with_type("png").ok()?;
    loader.write(&data[start..end]).ok()?;
    loader.close().ok()?;
    loader.pixbuf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    const PNG: &[u8] = include_bytes!("../../assets/star_gray.png");

    #[test]
    fn opened_descriptor_survives_path_replacement() {
        let root = std::env::temp_dir().join(format!("icon-fd-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("icon.svg");
        fs::write(&path, PNG).unwrap();
        let data = read_icon_bytes(&path).unwrap();
        fs::rename(&path, root.join("original.png")).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(decode_icon_bytes(&data).is_some());
        assert!(read_icon_bytes(&path).is_none());
        assert!(read_icon_bytes(Path::new("/dev/null")).is_none());
    }

    #[test]
    fn symlink_and_ico_decode_keep_content_sniffing() {
        let root = std::env::temp_dir().join(format!("icon-symlink-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("content.svg"), PNG).unwrap();
        symlink("content.svg", root.join("image.ico")).unwrap();
        let data = read_icon_bytes(&root.join("image.ico")).unwrap();
        let expected = decode_icon_bytes(&data).unwrap();
        let mut ico = vec![0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 1, 0, 32, 0];
        ico.extend_from_slice(&u32::try_from(PNG.len()).unwrap().to_le_bytes());
        ico.extend_from_slice(&22_u32.to_le_bytes());
        ico.extend_from_slice(PNG);
        let actual = decode_ico_pixbuf(&ico).unwrap();
        assert_eq!(
            (actual.width(), actual.height()),
            (expected.width(), expected.height())
        );
        assert!(decode_ico_pixbuf(b"invalid").is_none());
    }
}
