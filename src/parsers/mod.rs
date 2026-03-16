pub mod definitions;
pub mod demangle;
pub mod goblin;
pub mod native;
pub mod nm;

use crate::parsers::definitions::ElfParser;
use crate::parsers::goblin::GoblinParser;
use crate::parsers::native::NativeParser;
use crate::parsers::nm::NmParser;
use eyre::Result;
use std::path::Path;

pub fn create_parser(parser_name: &str, _path: &Path) -> Result<Box<dyn ElfParser>> {
    match parser_name {
        "nm" => Ok(Box::new(NmParser::default())),
        "native" => Ok(Box::new(NativeParser)),
        "goblin" => Ok(Box::new(GoblinParser)),
        _ => eyre::bail!("Unknown parser type: {}", parser_name),
    }
}
