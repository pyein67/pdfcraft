//! The window PNG is the untouched largest frame of the contributor's multi-size ICO.
const ICO: &[u8] = include_bytes!("../../../assets/app-icon/korean/pdfcraft-pdf-icon-windows.ico");
const ORIGINAL: &[u8] = include_bytes!("../../../assets/app-icon/korean/pdfcraft-pdf-icon.ico");
const PNG: &[u8] = include_bytes!("../../../assets/app-icon/korean/pdfcraft-pdf-icon-256.png");

#[test]
fn supplied_windows_icon_has_all_shell_sizes_and_identical_window_frame() {
    assert_eq!(&ICO[..6], &[0, 0, 1, 0, 7, 0]);
    let mut sizes = Vec::new();
    for directory in ICO[6..118].as_chunks::<16>().0 {
        let size = u32::from_le_bytes(directory[8..12].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(directory[12..16].try_into().unwrap()) as usize;
        let frame = ICO.get(offset..offset.checked_add(size).unwrap()).unwrap();
        assert!(frame.starts_with(b"\x89PNG\r\n\x1a\n"));
        let width = if directory[0] == 0 { 256 } else { u32::from(directory[0]) };
        assert_eq!(width, u32::from_be_bytes(frame[16..20].try_into().unwrap()));
        assert_eq!(width, u32::from_be_bytes(frame[20..24].try_into().unwrap()));
        sizes.push(width);
        if width == 256 {
            assert_eq!(frame, PNG, "window/taskbar must use the supplied design without re-encoding");
        }
    }
    assert_eq!(sizes, [16, 24, 32, 48, 64, 128, 256]);
}

#[test]
fn windows_icon_directory_uses_single_color_plane() {
    for directory in ICO[6..118].as_chunks::<16>().0 {
        assert_eq!(
            u16::from_le_bytes(directory[4..6].try_into().unwrap()),
            1,
            "match the last working Windows icon's single-plane directory convention"
        );
    }
}

#[test]
fn compatibility_normalization_changes_only_plane_words_not_the_icon_design() {
    assert_eq!(ICO.len(), ORIGINAL.len());
    for (offset, (actual, original)) in ICO.iter().zip(ORIGINAL).enumerate() {
        let plane_byte = (0..7).any(|i| offset == 10 + 16 * i || offset == 11 + 16 * i);
        if !plane_byte {
            assert_eq!(actual, original, "byte {offset} changed");
        }
    }
}
