use std::{fs::File, io::{BufReader, BufWriter}, path::PathBuf, sync::Arc};

use glob::glob;
use shroom_img::{crypto::ImgCrypto, reader::ImgReader, value::Object};

fn main() -> anyhow::Result<()> {
    let files = glob("/home/jonas/shared_vm/maplestory/data/Mob/**/*.img");
    let out = PathBuf::from("out");
    std::fs::create_dir(&out)?;

    let crypto = Arc::new(ImgCrypto::global());
    for file in files? {
        let path = file?;
        let file = BufReader::new(File::open(&path)?);
        let mut img = ImgReader::new(file, crypto.clone().into());
        let obj = Object::from_reader(&mut img)?;


        let name = path.file_stem().unwrap().to_str().unwrap();
        let json = obj.to_json_value();
        let mut out = BufWriter::new(File::create(out.join(format!("{name}.json")))?);
        serde_json::to_writer_pretty(&mut out, &json)?;
    }

    Ok(())
}
