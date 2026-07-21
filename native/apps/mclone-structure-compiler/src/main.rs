use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use mclone_structure_compiler::StructurePreviewCompiler;

fn main() -> Result<()> {
    let options = Options::parse(env::args().skip(1))?;
    let compiler = StructurePreviewCompiler::load(&options.asset_pack_root)?;
    for structure_json in &options.structure_json {
        let receipt = compiler.compile_json_file(structure_json, &options.out_root)?;
        println!(
            "compiled {}: {} vertices, {} indices, {}",
            receipt.structure_id,
            receipt.geometry.vertex_count,
            receipt.geometry.index_count,
            options.out_root.display()
        );
    }
    Ok(())
}

#[derive(Debug)]
struct Options {
    asset_pack_root: PathBuf,
    out_root: PathBuf,
    structure_json: Vec<PathBuf>,
}

impl Options {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self> {
        let mut asset_pack_root = None;
        let mut out_root = None;
        let mut structure_json = Vec::new();
        let mut args = args.peekable();
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--asset-pack-root" => {
                    asset_pack_root = Some(PathBuf::from(
                        args.next().context("--asset-pack-root requires a path")?,
                    ));
                }
                "--out-root" => {
                    out_root = Some(PathBuf::from(
                        args.next().context("--out-root requires a path")?,
                    ));
                }
                value if value.starts_with('-') => bail!("unknown option `{value}`"),
                value => structure_json.push(PathBuf::from(value)),
            }
        }
        if structure_json.is_empty() {
            bail!(
                "usage: mclone-structure-compiler --asset-pack-root <dir> \
                 --out-root <dir> <structure.json>..."
            );
        }
        Ok(Self {
            asset_pack_root: asset_pack_root.context("missing --asset-pack-root")?,
            out_root: out_root.context("missing --out-root")?,
            structure_json,
        })
    }
}
