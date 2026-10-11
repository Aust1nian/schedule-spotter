//! Apple Numbers documents. Phase 1 adds the reader; until then the app
//! asks for an Excel export or a paste.

use std::io::Cursor;

use crate::model::ReadError;
use crate::read::Workbook;

pub fn read(_archive: zip::ZipArchive<Cursor<&[u8]>>, _name: &str) -> Result<Workbook, ReadError> {
    Err(ReadError::Unreadable)
}
