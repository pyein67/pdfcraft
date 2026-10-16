//! The window PNG is the untouched largest frame of the contributor's multi-size ICO.
const ICO: &[u8] = include_bytes!("../../../assets/app-icon/korean/pdfcraft-pdf-icon.ico");
const PNG: &[u8] = include_bytes!("../../../assets/app-icon/korean/pdfcraft-pdf-icon-256.png");

#[test]
fn supplied_windows_icon_has_all_shell_sizes_and_identical_window_frame() {
    assert_eq!(&ICO[..6], &[0, 0, 1, 0, 7, 0]);
    let mut sizes = Vec::new();
    for directory in ICO[6..118].chunks_exact(16) {
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
