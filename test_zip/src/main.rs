use std::io::{Read, Write, Seek};
fn main() {
    let tmp = std::env::temp_dir().join("test_read.zip");
    {
        let f = std::fs::File::create(&tmp).unwrap();
        let mut zip = zip::ZipWriter::new(f);
        zip.finish().unwrap();
    }
    
    let mut f2 = std::fs::File::open(&tmp).unwrap();
    let mut archive = zip::ZipArchive::new(f2).unwrap();
    println!("archive len: {}", archive.len());
}
