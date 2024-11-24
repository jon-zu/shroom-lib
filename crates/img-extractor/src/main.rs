use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
    sync::Arc,
};

use glob::glob;
use shroom_img::{crypto::ImgCrypto, reader::ImgReader, value::Object};

fn do_dump(path: impl AsRef<Path>, base: impl AsRef<Path>, crypto: &Arc<ImgCrypto>, out: &Path) -> anyhow::Result<()> {
    let path = path.as_ref();
    let file = BufReader::new(File::open(path)?);
    let mut img = ImgReader::new(file, crypto.clone().into());
    let obj = Object::from_reader(&mut img)?;

    let name = path.file_stem().unwrap().to_str().unwrap();
    let rel = path
        .strip_prefix(&base)
        .unwrap()
        .with_file_name(format!("{name}.json"));
    let path = out.join(&rel);
    std::fs::create_dir_all(path.parent().unwrap())?;

    let json = obj.to_json_value();
    let mut out = BufWriter::new(File::create(path)?);
    serde_json::to_writer_pretty(&mut out, &json)?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let base = PathBuf::from("/home/jonas/Downloads/bms/5366a09f4e67570decdbef93468edf19/DataSvr");
    let files = glob(&format!("{}/**/*.img", base.to_str().unwrap()));
    let out = PathBuf::from("out");
    std::fs::create_dir(&out)?;

    let crypto = Arc::new(ImgCrypto::none());
    for file in files? {
        dbg!(&file);
        let path = file?;
        let s = path.to_str().unwrap();
        if s.contains("Commodity.img") || s.contains("Character/") {
            continue;
        }

        if path.file_name().unwrap() == "Center.img" {
            continue;
        }

        if let Err(err) = do_dump(path, &base, &crypto, &out)  {
            println!("Error: {:?}", err);
        }
    }

    Ok(())
}
