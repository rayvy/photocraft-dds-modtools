//! Public command-path coverage: no need to compile every engine unit test for this regression.
use photocraft_engine::Session;
use serde_json::json;

#[test]
fn dds_source_defaults_survive_headless_save() {
    let temp = tempfile::tempdir().unwrap();
    let mut session = Session::new();
    session.execute("file.new", json!({"width":4,"height":4,"depth":8})).unwrap();
    let document = std::sync::Arc::make_mut(&mut session.active_mut().unwrap().doc);
    document.file_encoding.dds_format = Some(photocraft_io::DdsCompression::Bc3.storage_id());
    document.file_encoding.dds_mipmaps = Some(true);
    let path = temp.path().join("keep.dds");
    session.execute("file.saveACopy", json!({"path":path})).unwrap();
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(&bytes[84..88], b"DXT5");
    assert_eq!(u32::from_le_bytes(bytes[28..32].try_into().unwrap()), 3);
    let path = temp.path().join("no-mips.dds");
    session.execute("file.saveACopy", json!({"path":path,"ddsMipmaps":false})).unwrap();
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(&bytes[84..88], b"DXT5");
    assert_eq!(u32::from_le_bytes(bytes[28..32].try_into().unwrap()), 1);
}
